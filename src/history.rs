//! Space-saved history: one TSV line per cleaning run, no database.
//!
//! `~/.config/klean/history.tsv` (override with `KLEAN_HISTORY`, used by tests).
//! Columns: epoch, action, freed_bytes, items, projects, root.
//! ponytail: append-only TSV, grep/awk-able; move to SQLite past ~100k runs.

use crate::config::dirs;
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn history_path() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("KLEAN_HISTORY") {
        if !custom.is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }
    let config = dirs::config_dir().context("cannot determine the config directory")?;
    Ok(config.join("klean").join("history.tsv"))
}

#[derive(Debug, Clone)]
pub struct Run {
    pub at: i64,
    pub action: String,
    pub freed: u64,
    pub items: usize,
    pub projects: usize,
    pub root: String,
}

/// Append one cleaning run. Never fails the caller over history bookkeeping.
pub fn record(root: &Path, action: &str, freed: u64, items: usize, projects: usize) -> Result<()> {
    let path = history_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context(format!("cannot create {:?}", parent))?;
    }

    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the unix epoch")?
        .as_secs() as i64;

    let line = format!(
        "{at}\t{action}\t{freed}\t{items}\t{projects}\t{}\n",
        root.display()
    );

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

pub fn read_all(path: &Path) -> Vec<Run> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };

    text.lines()
        .filter_map(|line| {
            // splitn(6) so a path containing a tab still survives as the root.
            let mut parts = line.splitn(6, '\t');
            let at = parts.next()?.parse().ok()?;
            let action = parts.next()?.to_string();
            let freed = parts.next()?.parse().ok()?;
            let items = parts.next()?.parse().ok()?;
            let projects = parts.next()?.parse().ok()?;
            let root = parts.next().unwrap_or_default().to_string();
            Some(Run {
                at,
                action,
                freed,
                items,
                projects,
                root,
            })
        })
        .collect()
}

fn timestamp(at: i64) -> String {
    match chrono::DateTime::from_timestamp(at, 0) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        None => at.to_string(),
    }
}

fn size(bytes: u64) -> String {
    humansize::format_size(bytes, humansize::BINARY)
}

pub fn print_stats(path: &Path, json: bool) -> Result<()> {
    let runs = read_all(path);
    let total: u64 = runs.iter().map(|r| r.freed).sum();

    // Per project, biggest first.
    let mut projects: Vec<(String, u64, usize)> = Vec::new();
    for run in &runs {
        match projects.iter_mut().find(|(root, _, _)| *root == run.root) {
            Some(entry) => {
                entry.1 += run.freed;
                entry.2 += 1;
            }
            None => projects.push((run.root.clone(), run.freed, 1)),
        }
    }
    projects.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    if json {
        let doc = serde_json::json!({
            "history_file": path.display().to_string(),
            "runs": runs.len(),
            "total_freed_bytes": total,
            "projects": projects
                .iter()
                .map(|(root, freed, count)| serde_json::json!({
                    "root": root,
                    "freed_bytes": freed,
                    "runs": count,
                }))
                .collect::<Vec<_>>(),
            "recent": runs
                .iter()
                .rev()
                .take(10)
                .map(|run| serde_json::json!({
                    "at": run.at,
                    "at_human": timestamp(run.at),
                    "action": run.action,
                    "freed_bytes": run.freed,
                    "items": run.items,
                    "projects": run.projects,
                    "root": run.root,
                }))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }

    if runs.is_empty() {
        println!("📈 Sem histórico ainda ({})", path.display());
        println!("   Cada limpeza real grava uma linha aqui.");
        return Ok(());
    }

    println!(
        "📈 klean — {} execução(ões) — {}",
        runs.len(),
        path.display()
    );
    println!("{:-<70}", "");
    println!("  Total liberado: {}", size(total));
    println!("  Média por run:  {}", size(total / runs.len() as u64));
    if let Some(biggest) = runs.iter().max_by_key(|r| r.freed) {
        println!(
            "  Maior run:      {} ({}, {})",
            size(biggest.freed),
            timestamp(biggest.at),
            biggest.root
        );
    }

    println!("\n  Por projeto:");
    for (root, freed, count) in projects.iter().take(15) {
        println!("    {:>12}  {}  ({} run(s))", size(*freed), root, count);
    }

    println!("\n  Últimos runs:");
    for run in runs.iter().rev().take(10) {
        println!(
            "    {}  {:>12}  {:>4} item(ns)  {:<7}  {}",
            timestamp(run.at),
            size(run.freed),
            run.items,
            run.action,
            run.root
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_reads_back() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("history.tsv");
        std::env::set_var("KLEAN_HISTORY", &file);

        record(Path::new("/tmp/project"), "delete", 4096, 3, 1).unwrap();
        record(Path::new("/tmp/project"), "backup", 1024, 1, 1).unwrap();

        let runs = read_all(&file);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].freed, 4096);
        assert_eq!(runs[1].action, "backup");
        assert_eq!(runs[0].root, "/tmp/project");
        assert_eq!(runs.iter().map(|r| r.freed).sum::<u64>(), 5120);

        std::env::remove_var("KLEAN_HISTORY");
    }
}
