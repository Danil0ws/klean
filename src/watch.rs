//! Watch mode: rescan on an interval, optionally cleaning automatically.

use crate::cleaner::{Cleaner, CleanerAction};
use crate::scanner::ArtifactScanner;
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::time::Duration;

pub struct WatchOptions {
    pub interval: Duration,
    pub auto_clean: bool,
    pub once: bool,
    pub quiet: bool,
    pub allow_system_paths: bool,
    pub backup_dir: Option<PathBuf>,
}

/// Parse "30s", "15m", "2h", "1d" (or a bare number of seconds).
pub fn parse_duration(text: &str) -> Result<Duration> {
    let text = text.trim();
    if text.is_empty() {
        bail!("empty duration");
    }

    let (digits, unit_secs) = match text.chars().last().unwrap().to_ascii_lowercase() {
        's' => (&text[..text.len() - 1], 1u64),
        'm' => (&text[..text.len() - 1], 60),
        'h' => (&text[..text.len() - 1], 60 * 60),
        'd' => (&text[..text.len() - 1], 24 * 60 * 60),
        _ => (text, 1),
    };

    let amount: u64 = digits
        .trim()
        .parse()
        .context(format!("invalid duration: {text}"))?;

    if amount == 0 {
        bail!("duration must be greater than zero");
    }

    Ok(Duration::from_secs(amount * unit_secs))
}

pub fn run(scanner: &ArtifactScanner, options: WatchOptions) -> Result<()> {
    let mut round = 0u64;

    loop {
        round += 1;
        let outcome = scanner.scan()?;
        let total: u64 = outcome.artifacts.iter().map(|a| a.size).sum();
        let projects = outcome.groups().len();

        if !options.quiet {
            println!(
                "[watch #{round}] {} artifact(s) in {} project(s) — {}",
                outcome.artifacts.len(),
                projects,
                humansize::format_size(total, humansize::BINARY)
            );
            if !outcome.blocked.is_empty() {
                println!(
                    "  🚫 {} bloqueado(s) por regra de projeto",
                    outcome.blocked.len()
                );
            }
        }

        if options.auto_clean && !outcome.artifacts.is_empty() {
            let backup = options.backup_dir.is_some();
            let action = if backup {
                CleanerAction::Backup
            } else {
                CleanerAction::Delete
            };
            let cleaner = Cleaner::new(
                action,
                options.backup_dir.clone(),
                options.allow_system_paths,
            )
            .with_root(scanner.root());
            cleaner.verify_safety(&outcome.artifacts)?;
            let result = cleaner.clean(outcome.artifacts, false)?;

            if !options.quiet {
                println!(
                    "  🧹 {} item(s), {} liberado(s)",
                    result.deleted + result.backed_up,
                    humansize::format_size(result.total_size_freed, humansize::BINARY)
                );
            }

            crate::history::record(
                std::path::Path::new("watch"),
                if backup { "backup" } else { "delete" },
                result.total_size_freed,
                result.deleted + result.backed_up,
                projects,
            )?;
        } else if !options.quiet && !outcome.artifacts.is_empty() {
            println!("  (dry: rode com --auto-clean para remover)");
        }

        if options.once {
            return Ok(());
        }

        std::thread::sleep(options.interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("30").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("15m").unwrap(), Duration::from_secs(900));
        assert_eq!(parse_duration("2H").unwrap(), Duration::from_secs(7200));
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(86400));
        assert!(parse_duration("0m").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("").is_err());
    }
}
