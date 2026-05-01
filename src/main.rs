mod cleaner;
mod cli;
mod config;
mod ignore;
mod patterns;
mod scanner;
mod tui;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber;

use cleaner::{Cleaner, CleanerAction};
use cli::{Cli, Mode};
use config::KleanConfig;
use ignore::IgnoreRules;
use patterns::get_default_patterns;
use scanner::{parse_size, ArtifactScanner};
use tui::InteractiveMode;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    setup_logging(&cli);

    if cli.show_config {
        show_config(&cli)?;
        return Ok(());
    }

    let root = std::fs::canonicalize(&cli.get_path()).context("Failed to resolve root path")?;

    // Load configuration
    let local_config = KleanConfig::from_local(&root)?;
    let global_config = KleanConfig::from_global()?;

    let mut final_config = KleanConfig::default();
    if let Some(gc) = global_config {
        final_config = final_config.merge(&gc);
    }
    if let Some(lc) = local_config {
        final_config = final_config.merge(&lc);
    }

    // Load ignore rules
    let ignore_rules = if let Some(klignore_path) = &cli.klignore {
        IgnoreRules::from_custom_file(klignore_path)?
    } else {
        IgnoreRules::from_path(&root, cli.should_respect_gitignore())?
    };

    // Prepare patterns
    let mut patterns = get_default_patterns();
    if let Some(custom) = &final_config.patterns {
        patterns.extend(custom.clone());
    }

    // Create scanner
    let mut scanner = ArtifactScanner::new(root.clone(), ignore_rules, patterns);

    scanner = scanner.with_filter(cli.filter);

    if let Some(min_size_str) = &cli.min_size {
        let min_size = parse_size(min_size_str)?;
        scanner = scanner.with_size_limits(Some(min_size), None);
    }

    if let Some(max_size_str) = &cli.max_size {
        let max_size = parse_size(max_size_str)?;
        scanner = scanner.with_size_limits(None, Some(max_size));
    }

    // Scan for artifacts
    if !cli.quiet {
        println!("🔍 Scanning {}...", root.display());
    }

    let artifacts = scanner.scan()?;

    if artifacts.is_empty() {
        println!("✨ No artifacts found!");
        return Ok(());
    }

    if !cli.quiet {
        println!("📦 Found {} artifacts\n", artifacts.len());
    }

    // Determine mode
    let mode = cli.mode.clone().unwrap_or(Mode::Interactive);

    let selected_artifacts = match mode {
        Mode::Interactive if !cli.yes && !cli.dry_run => {
            // Run interactive TUI
            InteractiveMode::run(artifacts)?
        }
        Mode::List => {
            // Just list artifacts
            list_artifacts(&artifacts)?;
            None
        }
        _ => {
            // CLI mode or --yes flag
            if cli.yes || cli.dry_run {
                Some(artifacts)
            } else {
                list_artifacts(&artifacts)?;
                None
            }
        }
    };

    if let Some(to_clean) = selected_artifacts {
        if !to_clean.is_empty() {
            let action = if let Some(backup_dir) = &final_config.backup_dir {
                if !cli.quiet {
                    println!("📦 Backing up to {}", backup_dir.display());
                }
                CleanerAction::Backup
            } else {
                CleanerAction::Delete
            };

            let cleaner = Cleaner::new(action, final_config.backup_dir);
            cleaner.verify_safety(&to_clean)?;

            let result = cleaner.clean(to_clean, cli.dry_run)?;
            result.print_summary();
        }
    }

    Ok(())
}

fn list_artifacts(artifacts: &[scanner::Artifact]) -> Result<()> {
    println!("📋 Artifacts found:");
    println!("{:-<70}", "");
    for artifact in artifacts {
        println!(
            "  {} ({}) - {}",
            artifact.name,
            artifact.size_string(),
            artifact.path.display()
        );
    }
    println!("{:-<70}", "");
    Ok(())
}

fn show_config(cli: &Cli) -> Result<()> {
    println!("📋 Configuration:");
    println!("  Path: {:?}", cli.get_path());
    println!("  Dry run: {}", cli.dry_run);
    println!("  Yes: {}", cli.yes);
    println!("  Filter: {:?}", cli.filter);
    println!("  Min size: {:?}", cli.min_size);
    println!("  Max size: {:?}", cli.max_size);
    println!("  Klignore: {:?}", cli.klignore);
    println!("  Respect gitignore: {}", cli.should_respect_gitignore());
    println!("  Backup dir: {:?}", cli.backup_dir);
    println!("  Mode: {:?}", cli.mode);
    Ok(())
}

fn setup_logging(cli: &Cli) {
    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    if !cli.quiet {
        tracing_subscriber::fmt()
            .with_max_level(level.parse().unwrap_or(tracing::Level::WARN))
            .init();
    }
}
