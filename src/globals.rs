//! Global package scanner and uninstaller for npm, cargo, and pip.

use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Manager {
    Npm,
    Cargo,
    Pip,
}

impl Manager {
    pub fn label(&self) -> &'static str {
        match self {
            Manager::Npm => "npm",
            Manager::Cargo => "cargo",
            Manager::Pip => "pip",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Manager::Npm => "📦",
            Manager::Cargo => "🦀",
            Manager::Pip => "🐍",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlobalPackage {
    pub name: String,
    pub version: String,
    pub manager: Manager,
    /// Disk usage in bytes (0 when unknown/too expensive to compute).
    pub size_bytes: u64,
}

impl GlobalPackage {
    pub fn size_string(&self) -> String {
        if self.size_bytes == 0 {
            return "—".to_string();
        }
        humansize::format_size(self.size_bytes, humansize::BINARY)
    }
}

/// Scan for global packages from all available managers.
pub fn scan_all() -> Vec<GlobalPackage> {
    let mut packages = Vec::new();
    packages.extend(scan_npm());
    packages.extend(scan_cargo());
    packages.extend(scan_pip());
    packages
}

// ── npm ──────────────────────────────────────────────────────────────────────

fn scan_npm() -> Vec<GlobalPackage> {
    let npm_root: Option<PathBuf> = Command::new("npm")
        .args(["root", "-g"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| PathBuf::from(s.trim()))
            } else {
                None
            }
        });

    let out = match Command::new("npm")
        .args(["list", "-g", "--depth=0", "--json"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if out.stdout.is_empty() {
        return Vec::new();
    }

    #[derive(Deserialize)]
    struct NpmList {
        dependencies: Option<HashMap<String, NpmDep>>,
    }
    #[derive(Deserialize)]
    struct NpmDep {
        version: Option<String>,
    }

    let list: NpmList = match serde_json::from_slice(&out.stdout) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };

    list.dependencies
        .unwrap_or_default()
        .into_iter()
        .filter(|(name, _)| name != "npm")
        .map(|(name, dep)| {
            let size_bytes = npm_root
                .as_ref()
                .map(|r| dir_size(&r.join(&name)))
                .unwrap_or(0);
            GlobalPackage {
                name,
                version: dep.version.unwrap_or_else(|| "?".to_string()),
                manager: Manager::Npm,
                size_bytes,
            }
        })
        .collect()
}

// ── cargo ─────────────────────────────────────────────────────────────────────

fn flush_cargo_package(
    name: &str,
    version: &str,
    bins: &[String],
    cargo_bin: &Option<PathBuf>,
    packages: &mut Vec<GlobalPackage>,
) {
    if name.is_empty() {
        return;
    }
    let size_bytes: u64 = match cargo_bin {
        Some(dir) => bins
            .iter()
            .map(|b| dir.join(b).metadata().map(|m| m.len()).unwrap_or(0))
            .sum(),
        None => 0,
    };
    packages.push(GlobalPackage {
        name: name.to_string(),
        version: version.to_string(),
        manager: Manager::Cargo,
        size_bytes,
    });
}

fn scan_cargo() -> Vec<GlobalPackage> {
    let out = match Command::new("cargo").args(["install", "--list"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let cargo_bin: Option<PathBuf> = home_dir().map(|h| h.join(".cargo").join("bin"));
    let text = String::from_utf8_lossy(&out.stdout);

    let mut packages = Vec::new();
    let mut current_name = String::new();
    let mut current_version = String::new();
    let mut current_bins: Vec<String> = Vec::new();

    for line in text.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            flush_cargo_package(
                &current_name,
                &current_version,
                &current_bins,
                &cargo_bin,
                &mut packages,
            );
            current_bins.clear();
            // parse "name vX.Y.Z:"
            let trimmed = line.trim_end_matches(':');
            if let Some((name, ver)) = trimmed.rsplit_once(' ') {
                current_name = name.to_string();
                current_version = ver.trim_start_matches('v').to_string();
            }
        } else if line.starts_with("    ") {
            current_bins.push(line.trim().to_string());
        }
    }
    flush_cargo_package(
        &current_name,
        &current_version,
        &current_bins,
        &cargo_bin,
        &mut packages,
    );

    packages
}

// ── pip ───────────────────────────────────────────────────────────────────────

fn scan_pip() -> Vec<GlobalPackage> {
    #[derive(Deserialize)]
    struct PipPkg {
        name: String,
        version: String,
    }

    let out = Command::new("pip3")
        .args(["list", "--format=json"])
        .output()
        .or_else(|_| Command::new("pip").args(["list", "--format=json"]).output());

    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() || out.stdout.is_empty() {
        return Vec::new();
    }

    let list: Vec<PipPkg> = match serde_json::from_slice(&out.stdout) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };

    const SKIP: &[&str] = &["pip", "setuptools", "wheel", "pkg_resources", "distribute"];
    list.into_iter()
        .filter(|p| !SKIP.contains(&p.name.to_lowercase().as_str()))
        .map(|p| GlobalPackage {
            name: p.name,
            version: p.version,
            manager: Manager::Pip,
            size_bytes: 0,
        })
        .collect()
}

// ── uninstall ─────────────────────────────────────────────────────────────────

pub fn uninstall(pkg: &GlobalPackage, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    match pkg.manager {
        Manager::Npm => {
            let status = Command::new("npm")
                .args(["uninstall", "-g", &pkg.name])
                .status()?;
            if !status.success() {
                bail!("npm uninstall -g {} failed", pkg.name);
            }
        }
        Manager::Cargo => {
            let status = Command::new("cargo")
                .args(["uninstall", &pkg.name])
                .status()?;
            if !status.success() {
                bail!("cargo uninstall {} failed", pkg.name);
            }
        }
        Manager::Pip => {
            let status = Command::new("pip3")
                .args(["uninstall", "-y", &pkg.name])
                .status()
                .or_else(|_| {
                    Command::new("pip")
                        .args(["uninstall", "-y", &pkg.name])
                        .status()
                })?;
            if !status.success() {
                bail!("pip uninstall {} failed", pkg.name);
            }
        }
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let Ok(rd) = std::fs::read_dir(path) else {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    };
    let mut total = 0u64;
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_symlink() {
            total += p.symlink_metadata().map(|m| m.len()).unwrap_or(0);
        } else if p.is_file() {
            total += p.metadata().map(|m| m.len()).unwrap_or(0);
        } else if p.is_dir() {
            total += dir_size(&p);
        }
    }
    total
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}
