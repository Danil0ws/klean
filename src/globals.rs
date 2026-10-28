//! Global package scanner, updater and uninstaller.

use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Manager {
    Brew,
    Cargo,
    Npm,
    Pip,
    Binary,
}

impl Manager {
    pub fn label(&self) -> &'static str {
        match self {
            Manager::Npm => "npm",
            Manager::Cargo => "cargo",
            Manager::Pip => "pip",
            Manager::Brew => "brew",
            Manager::Binary => "binary",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Manager::Npm => "📦",
            Manager::Cargo => "🦀",
            Manager::Pip => "🐍",
            Manager::Brew => "🍺",
            Manager::Binary => "⚙️",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlobalPackage {
    pub name: String,
    pub version: String,
    pub manager: Manager,
    /// Disk usage in bytes (0 when unknown).
    pub size_bytes: u64,
    /// Latest version available (Some only after check_updates is run).
    pub update_available: Option<String>,
}

impl GlobalPackage {
    pub fn size_string(&self) -> String {
        if self.size_bytes == 0 {
            return "—".to_string();
        }
        humansize::format_size(self.size_bytes, humansize::BINARY)
    }

    /// Key used in the updates map: "manager:name".
    pub fn update_key(&self) -> String {
        format!("{}:{}", self.manager.label(), self.name)
    }
}

/// Scan for global packages from all available managers.
pub fn scan_all() -> Vec<GlobalPackage> {
    let mut packages = Vec::new();
    packages.extend(scan_npm());
    packages.extend(scan_cargo());
    packages.extend(scan_pip());
    packages.extend(scan_brew());
    packages.extend(scan_binaries());
    packages
}

/// Check for available updates. Returns map of update_key → latest version.
/// Covers npm, pip, and brew (cargo requires cargo-update crate).
pub fn check_updates(packages: &[GlobalPackage]) -> HashMap<String, String> {
    let mut result = HashMap::new();
    if packages.iter().any(|p| p.manager == Manager::Npm) {
        result.extend(check_npm_updates());
    }
    if packages.iter().any(|p| p.manager == Manager::Pip) {
        result.extend(check_pip_updates());
    }
    if packages.iter().any(|p| p.manager == Manager::Brew) {
        result.extend(check_brew_updates());
    }
    result
}

/// Uninstall a package (noop when dry_run is true).
pub fn uninstall(pkg: &GlobalPackage, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    match pkg.manager {
        Manager::Npm => run("npm", &["uninstall", "-g", &pkg.name]),
        Manager::Cargo => run("cargo", &["uninstall", &pkg.name]),
        Manager::Pip => run("pip3", &["uninstall", "-y", &pkg.name])
            .or_else(|_| run("pip", &["uninstall", "-y", &pkg.name])),
        Manager::Brew => run("brew", &["uninstall", &pkg.name]),
        Manager::Binary => {
            let home = home_dir().unwrap_or_else(|| PathBuf::from("/"));
            let candidates = [
                home.join(".local").join("bin").join(&pkg.name),
                home.join("bin").join(&pkg.name),
            ];
            for path in &candidates {
                if path.exists() {
                    std::fs::remove_file(path)?;
                    return Ok(());
                }
            }
            bail!("binary {} not found in ~/.local/bin or ~/bin", pkg.name)
        }
    }
}

/// Update a package to the latest version.
pub fn update_package(pkg: &GlobalPackage, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    match pkg.manager {
        Manager::Npm => run("npm", &["install", "-g", &pkg.name]),
        Manager::Cargo => run("cargo", &["install", &pkg.name]),
        Manager::Pip => run("pip3", &["install", "--upgrade", &pkg.name])
            .or_else(|_| run("pip", &["install", "--upgrade", &pkg.name])),
        Manager::Brew => run("brew", &["upgrade", &pkg.name]),
        Manager::Binary => bail!("auto-update not supported for binary packages"),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn run(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        bail!("{} {} failed", cmd, args.join(" "));
    }
    Ok(())
}

fn make_pkg(name: String, version: String, manager: Manager, size_bytes: u64) -> GlobalPackage {
    GlobalPackage {
        name,
        version,
        manager,
        size_bytes,
        update_available: None,
    }
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
            make_pkg(
                name,
                dep.version.unwrap_or_else(|| "?".to_string()),
                Manager::Npm,
                size_bytes,
            )
        })
        .collect()
}

fn check_npm_updates() -> HashMap<String, String> {
    // npm outdated exits with code 1 when packages ARE outdated (normal)
    let out = match Command::new("npm")
        .args(["outdated", "-g", "--json"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return HashMap::new(),
    };
    if out.stdout.is_empty() {
        return HashMap::new();
    }

    #[derive(Deserialize)]
    struct NpmOutdated {
        latest: Option<String>,
    }

    let map: HashMap<String, NpmOutdated> = match serde_json::from_slice(&out.stdout) {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };

    map.into_iter()
        .filter_map(|(name, info)| info.latest.map(|v| (format!("npm:{name}"), v)))
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
    packages.push(make_pkg(
        name.to_string(),
        version.to_string(),
        Manager::Cargo,
        size_bytes,
    ));
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
        .map(|p| make_pkg(p.name, p.version, Manager::Pip, 0))
        .collect()
}

fn check_pip_updates() -> HashMap<String, String> {
    let out = Command::new("pip3")
        .args(["list", "--outdated", "--format=json"])
        .output()
        .or_else(|_| {
            Command::new("pip")
                .args(["list", "--outdated", "--format=json"])
                .output()
        });

    let Ok(out) = out else { return HashMap::new() };
    if out.stdout.is_empty() {
        return HashMap::new();
    }

    #[derive(Deserialize)]
    struct PipOutdated {
        name: String,
        latest_version: String,
    }

    let list: Vec<PipOutdated> = match serde_json::from_slice(&out.stdout) {
        Ok(l) => l,
        Err(_) => return HashMap::new(),
    };

    list.into_iter()
        .map(|p| (format!("pip:{}", p.name), p.latest_version))
        .collect()
}

// ── brew ──────────────────────────────────────────────────────────────────────

fn scan_brew() -> Vec<GlobalPackage> {
    let out = match Command::new("brew")
        .args(["list", "--formula", "--versions"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?.to_string();
            let version = parts.next().unwrap_or("?").to_string();
            let size = brew_size(&name);
            Some(make_pkg(name, version, Manager::Brew, size))
        })
        .collect()
}

fn brew_size(name: &str) -> u64 {
    for cellar in &["/opt/homebrew/Cellar", "/usr/local/Cellar"] {
        let p = PathBuf::from(cellar).join(name);
        if p.exists() {
            return dir_size(&p);
        }
    }
    0
}

fn check_brew_updates() -> HashMap<String, String> {
    let out = match Command::new("brew")
        .args(["outdated", "--json=v2"])
        .output()
    {
        Ok(o) if !o.stdout.is_empty() => o,
        _ => return HashMap::new(),
    };

    #[derive(Deserialize)]
    struct Root {
        formulae: Vec<Formula>,
    }
    #[derive(Deserialize)]
    struct Formula {
        name: String,
        current_version: String,
    }

    let data: Root = match serde_json::from_slice(&out.stdout) {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };

    data.formulae
        .into_iter()
        .map(|f| (format!("brew:{}", f.name), f.current_version))
        .collect()
}

// ── binary scanner ────────────────────────────────────────────────────────────

fn scan_binaries() -> Vec<GlobalPackage> {
    let home = match home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let scan_dirs = [home.join(".local").join("bin"), home.join("bin")];
    let exclude_dirs = managed_bin_dirs(&home);

    let mut packages = Vec::new();
    for dir in &scan_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if !n.starts_with('.') => n.to_string(),
                _ => continue,
            };
            if !is_executable(&path) || is_in_managed_dir(&path, &exclude_dirs) {
                continue;
            }
            let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
            let version = get_binary_version(&path);
            packages.push(make_pkg(name, version, Manager::Binary, size_bytes));
        }
    }

    packages
}

fn managed_bin_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        home.join(".cargo").join("bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/opt/homebrew/sbin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Ok(out) = Command::new("npm").args(["bin", "-g"]).output() {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                dirs.push(PathBuf::from(s.trim()));
            }
        }
    }
    dirs
}

fn is_in_managed_dir(path: &Path, managed: &[PathBuf]) -> bool {
    let real = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    for dir in managed {
        if real.starts_with(dir) {
            return true;
        }
        if let Ok(rd) = dir.canonicalize() {
            if real.starts_with(&rd) {
                return true;
            }
        }
    }
    false
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.extension().map(|e| e == "exe").unwrap_or(false)
    }
}

fn get_binary_version(path: &Path) -> String {
    for flag in &["--version", "-V"] {
        if let Ok(out) = Command::new(path).arg(flag).output() {
            let text = if !out.stdout.is_empty() {
                String::from_utf8_lossy(&out.stdout).into_owned()
            } else {
                String::from_utf8_lossy(&out.stderr).into_owned()
            };
            if let Some(line) = text.lines().next() {
                let v = extract_version(line.trim());
                if !v.is_empty() {
                    return v;
                }
            }
        }
    }
    "?".to_string()
}

fn extract_version(s: &str) -> String {
    for word in s.split_whitespace() {
        let w = word.trim_matches(|c: char| c == ',' || c == ')' || c == '(');
        let w = w.trim_start_matches('v');
        if w.chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
            && w.contains('.')
        {
            return w.to_string();
        }
    }
    String::new()
}

// ── misc helpers ──────────────────────────────────────────────────────────────

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
