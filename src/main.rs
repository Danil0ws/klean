use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use klean::cleaner::{CleanResult, Cleaner, CleanerAction};
use klean::cli::{Cli, Mode};
use klean::config::KleanConfig;
use klean::history;
use klean::ignore::IgnoreRules;
use klean::patterns::get_default_patterns;
use klean::plugins;
use klean::scanner::{parse_size, Artifact, ArtifactScanner, ProjectGroup, ScanOutcome};
use klean::tui::{CleanContext, InteractiveMode};
use klean::watch::{self, WatchOptions};
use klean::web::{self, WebConfig};
use std::io::IsTerminal;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Exit code used when --fail-if-over is exceeded (CI gate). 1 stays "error".
const EXIT_OVER_LIMIT: i32 = 2;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    setup_logging(&cli);

    if cli.show_config {
        show_config(&cli)?;
        return Ok(());
    }

    let mode = cli.mode.clone().unwrap_or(Mode::Interactive);

    // Modes that need no scan at all.
    match mode {
        Mode::Stats => {
            let path = history::history_path()?;
            return history::print_stats(&path, cli.json);
        }
        Mode::Plugins => return list_plugins(&cli),
        _ => {}
    }

    let root = std::fs::canonicalize(cli.get_path()).context("Failed to resolve root path")?;

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

    // Load ignore rules (CLI flag and klean.toml both gate .gitignore support)
    let respect_gitignore =
        cli.should_respect_gitignore() && final_config.respect_gitignore.unwrap_or(true);
    let ignore_rules = if let Some(klignore_path) = &cli.klignore {
        IgnoreRules::from_custom_file(klignore_path)?
    } else {
        IgnoreRules::from_path(&root, respect_gitignore)?
    };

    // Prepare patterns: built-ins + plugins + klean.toml
    let mut patterns = get_default_patterns();
    patterns.extend(plugins::patterns(&root));
    if let Some(custom) = &final_config.patterns {
        patterns.extend(custom.clone());
    }

    // Create scanner
    let mut scanner = ArtifactScanner::new(root.clone(), ignore_rules, patterns);

    scanner = scanner.with_filter(cli.filter.clone());
    scanner = scanner.with_allow_system_paths(cli.allow_system_paths);
    scanner = scanner.with_project_rules(final_config.projects.clone().unwrap_or_default());

    if let Some(min_size_str) = &cli.min_size {
        let min_size = parse_size(min_size_str)?;
        scanner = scanner.with_size_limits(Some(min_size), None);
    }

    if let Some(max_size_str) = &cli.max_size {
        let max_size = parse_size(max_size_str)?;
        scanner = scanner.with_size_limits(None, Some(max_size));
    }

    // Modes that own the scan loop themselves.
    if matches!(mode, Mode::Serve) {
        let token = cli
            .token
            .clone()
            .or_else(|| std::env::var("KLEAN_TOKEN").ok())
            .filter(|token| !token.is_empty());

        return web::serve(
            scanner,
            WebConfig {
                host: cli.host.clone().unwrap_or_else(|| "127.0.0.1".to_string()),
                port: cli.port,
                token,
                root,
                allow_system_paths: cli.allow_system_paths,
                backup_dir: final_config.backup_dir.clone(),
                quiet: cli.quiet,
            },
        );
    }

    if matches!(mode, Mode::Watch) {
        return watch::run(
            &scanner,
            WatchOptions {
                interval: watch::parse_duration(&cli.interval)?,
                auto_clean: cli.auto_clean || cli.yes,
                once: cli.once,
                quiet: cli.quiet,
                allow_system_paths: cli.allow_system_paths,
                backup_dir: final_config.backup_dir.clone(),
            },
        );
    }

    // Scan for artifacts, naming the directory being read while it happens.
    let is_tty = std::io::stdout().is_terminal();
    let spinner = if is_tty && !cli.quiet && !cli.json {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} lendo {msg}")
                .context("Failed to set spinner style")?,
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(90));
        bar.set_message(root.display().to_string());
        Some(bar)
    } else {
        if !cli.quiet && !cli.json {
            println!("🔍 Scanning {}...", root.display());
        }
        None
    };

    if let Some(bar) = spinner.clone() {
        let base = root.clone();
        let clock = std::time::Instant::now();
        let last = AtomicU64::new(0);
        scanner = scanner.with_progress(Box::new(move |path: &Path| {
            // ponytail: throttle the redraw; without it a big tree spends more
            // time formatting the line than walking it.
            let now = clock.elapsed().as_millis() as u64;
            let previous = last.load(Ordering::Relaxed);
            if now.saturating_sub(previous) < 80 {
                return;
            }
            last.store(now, Ordering::Relaxed);
            let shown = path.strip_prefix(&base).unwrap_or(path);
            bar.set_message(shown.display().to_string());
        }));
    }

    let outcome = scanner.scan()?;

    if let Some(bar) = &spinner {
        bar.finish_and_clear();
    }

    let over_limit = match &cli.fail_if_over {
        Some(text) => {
            let limit = parse_size(text)?;
            let total: u64 = outcome.artifacts.iter().map(|a| a.size).sum();
            if total > limit {
                eprintln!(
                    "❌ total {} acima do limite {}",
                    humansize::format_size(total, humansize::BINARY),
                    humansize::format_size(limit, humansize::BINARY)
                );
                true
            } else {
                false
            }
        }
        None => false,
    };

    if cli.json {
        print_scan_json(&outcome, &root)?;
    }

    let ScanOutcome { artifacts, blocked } = outcome;

    if !blocked.is_empty() && !cli.quiet && !cli.json {
        println!(
            "🚫 {} artifact(s) skipped by project rules ({}); use -v to list them",
            blocked.len(),
            blocked
                .iter()
                .map(|b| b.project.display().to_string())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        );
        if cli.verbose > 0 {
            for item in &blocked {
                println!(
                    "   - {} ({}) — {}",
                    item.path.display(),
                    item.name,
                    item.reason
                );
            }
        }
    }

    if artifacts.is_empty() {
        if !cli.json {
            println!("✨ No artifacts found!");
        }
        return finish(over_limit);
    }

    let groups = group_artifacts(&artifacts);

    if !cli.quiet && !cli.json {
        println!(
            "📦 Found {} artifacts in {} project(s) — {} total\n",
            artifacts.len(),
            groups.len(),
            humansize::format_size(
                artifacts.iter().map(|a| a.size).sum::<u64>(),
                humansize::BINARY
            )
        );
    }

    // No TTY (pipe, CI, cron): the interactive UI cannot run.
    let mode = if matches!(mode, Mode::Interactive) && !is_tty {
        Mode::List
    } else {
        mode
    };

    let selected_artifacts = match mode {
        Mode::Interactive if !cli.yes && !cli.dry_run => {
            // The TUI cleans on its own, so the numbers stay on the screen
            // instead of scrolling away when it closes.
            let report = InteractiveMode::run(
                artifacts.clone(),
                CleanContext {
                    backup_dir: final_config.backup_dir.clone(),
                    root: root.clone(),
                    allow_system_paths: cli.allow_system_paths,
                },
            )?;

            if !cli.quiet && report.cleaned > 0 {
                println!(
                    "✓ {} liberados em {} item(ns)",
                    humansize::format_size(report.freed, humansize::BINARY),
                    report.cleaned
                );
            }
            None
        }
        Mode::List => {
            if !cli.json {
                list_artifacts(&groups, &root)?;
            }
            None
        }
        _ => {
            // CLI mode or --yes flag
            if cli.yes || cli.dry_run {
                Some(artifacts)
            } else {
                if !cli.json {
                    list_artifacts(&groups, &root)?;
                }
                None
            }
        }
    };

    if let Some(to_clean) = selected_artifacts {
        if !to_clean.is_empty() {
            let project_count = group_artifacts(&to_clean).len();
            let action_label = if final_config.backup_dir.is_some() {
                "backup"
            } else {
                "delete"
            };
            let action = if let Some(backup_dir) = &final_config.backup_dir {
                if !cli.quiet && !cli.json {
                    println!("📦 Backing up to {}", backup_dir.display());
                }
                CleanerAction::Backup
            } else {
                CleanerAction::Delete
            };

            let cleaner = Cleaner::new(action, final_config.backup_dir, cli.allow_system_paths)
                .with_root(root.clone());
            cleaner.verify_safety(&to_clean)?;

            let result = cleaner.clean(to_clean, cli.dry_run)?;

            if cli.json {
                print_clean_json(&result, cli.dry_run)?;
            } else {
                result.print_summary();
            }

            // Only real runs go to the history: dry runs free nothing.
            if !cli.dry_run {
                history::record(
                    &root,
                    action_label,
                    result.total_size_freed,
                    result.deleted + result.backed_up,
                    project_count,
                )?;
            }
        }
    }

    finish(over_limit)
}

fn finish(over_limit: bool) -> Result<()> {
    if over_limit {
        std::process::exit(EXIT_OVER_LIMIT);
    }
    Ok(())
}

/// Machine-readable scan document (`--json`), for CI and the web UI.
fn print_scan_json(outcome: &ScanOutcome, root: &Path) -> Result<()> {
    let groups = outcome.groups();
    let total: u64 = outcome.artifacts.iter().map(|a| a.size).sum();

    let doc = serde_json::json!({
        "root": root.display().to_string(),
        "total_bytes": total,
        "total_human": humansize::format_size(total, humansize::BINARY),
        "artifact_count": outcome.artifacts.len(),
        "project_count": groups.len(),
        "projects": groups
            .iter()
            .map(|group| serde_json::json!({
                "root": group.root.display().to_string(),
                "total_bytes": group.total_size,
                "total_human": group.size_string(),
                "artifacts": group
                    .artifacts
                    .iter()
                    .map(|artifact| serde_json::json!({
                        "name": artifact.name,
                        "pattern": artifact.pattern_name,
                        "path": artifact.path.display().to_string(),
                        "relative_path": artifact.relative_to(root).display().to_string(),
                        "size_bytes": artifact.size,
                        "size_human": artifact.size_string(),
                        "safe": artifact.is_safe,
                    }))
                    .collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
        "blocked": outcome
            .blocked
            .iter()
            .map(|blocked| serde_json::json!({
                "path": blocked.path.display().to_string(),
                "name": blocked.name,
                "project": blocked.project.display().to_string(),
                "reason": blocked.reason,
            }))
            .collect::<Vec<_>>(),
    });

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

fn print_clean_json(result: &CleanResult, dry_run: bool) -> Result<()> {
    let doc = serde_json::json!({
        "dry_run": dry_run,
        "deleted": result.deleted,
        "backed_up": result.backed_up,
        "failed": result.failed,
        "freed_bytes": result.total_size_freed,
        "freed_human": humansize::format_size(result.total_size_freed, humansize::BINARY),
        "errors": result.errors,
    });
    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

/// `klean plugins`: where plugins are looked up and what loaded.
fn list_plugins(cli: &Cli) -> Result<()> {
    let root = std::fs::canonicalize(cli.get_path()).context("Failed to resolve root path")?;
    let loaded = plugins::load(&root);

    println!("🧩 klean plugins");
    println!("{:-<70}", "");
    for dir in plugins::plugin_dirs(&root) {
        println!(
            "  dir: {}{}",
            dir.display(),
            if dir.exists() { "" } else { "  (não existe)" }
        );
    }

    if loaded.is_empty() {
        println!(
            "\n  nenhum plugin carregado ({} patterns embutidos)",
            get_default_patterns().len()
        );
        println!("  crie um .toml em qualquer dir acima para adicionar tipos de artefato");
        return Ok(());
    }

    println!();
    for plugin in &loaded {
        println!("  {}", plugin.source.display());
        for pattern in &plugin.patterns {
            println!(
                "      {} -> {:?} [{}]",
                pattern.name,
                pattern.patterns,
                pattern.languages.join(", ")
            );
        }
    }

    println!(
        "\n  {} pattern(s) de plugin + {} embutidos",
        loaded.iter().map(|p| p.patterns.len()).sum::<usize>(),
        get_default_patterns().len()
    );
    Ok(())
}

/// Group artifacts by their owning project, largest project first.
fn group_artifacts(artifacts: &[Artifact]) -> Vec<ProjectGroup> {
    ScanOutcome {
        artifacts: artifacts.to_vec(),
        blocked: Vec::new(),
    }
    .groups()
}

fn list_artifacts(groups: &[ProjectGroup], root: &Path) -> Result<()> {
    const NAME_WIDTH: usize = 24;
    const SIZE_WIDTH: usize = 10;

    println!("📋 Artifacts by project (largest first):");
    println!("{:-<78}", "");

    for group in groups {
        let project_name = group
            .root
            .strip_prefix(root)
            .ok()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| group.root.display().to_string());

        println!(
            "\n▸ {}  ({}, {} item{})",
            project_name,
            group.size_string(),
            group.artifacts.len(),
            if group.artifacts.len() == 1 { "" } else { "s" }
        );

        for artifact in &group.artifacts {
            // Show the pattern name only when it adds information.
            let label = if artifact.pattern_name == artifact.name {
                artifact.name.clone()
            } else {
                format!("{} [{}]", artifact.name, artifact.pattern_name)
            };
            println!(
                "    {:<name_width$} {:>size_width$}  {}",
                truncate(&label, NAME_WIDTH),
                artifact.size_string(),
                artifact.relative_to(root).display(),
                name_width = NAME_WIDTH,
                size_width = SIZE_WIDTH,
            );
        }
    }

    println!("\n{:-<78}", "");
    Ok(())
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
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
    println!("  Allow system paths: {}", cli.allow_system_paths);
    println!("  Backup dir: {:?}", cli.backup_dir);
    println!("  Mode: {:?}", cli.mode);
    println!("  JSON: {}", cli.json);
    println!("  Fail if over: {:?}", cli.fail_if_over);
    println!("  Host/port: {:?}:{}", cli.host, cli.port);
    println!("  Interval: {}", cli.interval);
    println!("  Auto clean: {}", cli.auto_clean);
    println!("  Once: {}", cli.once);
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
