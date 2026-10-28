//! Self-update: check GitHub releases and replace the running binary.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const REPO: &str = "Danil0ws/klean";
const CRATE: &str = "klean";

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub latest: String,
    pub current: String,
    pub update_available: bool,
    pub release_url: String,
}

impl VersionInfo {
    pub fn fetch() -> Result<Self> {
        let current = env!("CARGO_PKG_VERSION").to_string();
        let latest = fetch_latest_tag()?;
        let update_available = is_newer(&latest, &current);
        Ok(Self {
            release_url: format!("https://github.com/{REPO}/releases/tag/v{latest}"),
            latest,
            current,
            update_available,
        })
    }
}

/// Returns `true` when `a` is semantically newer than `b`.
fn is_newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> (u64, u64, u64) {
        let s = s.trim_start_matches('v');
        let mut parts = s.splitn(3, '.').map(|p| p.parse::<u64>().unwrap_or(0));
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    };
    parse(a) > parse(b)
}

/// Fetch the latest published version from crates.io.
fn fetch_latest_tag() -> Result<String> {
    #[derive(Deserialize)]
    struct Crate {
        #[serde(rename = "crate")]
        inner: CrateInner,
    }
    #[derive(Deserialize)]
    struct CrateInner {
        newest_version: String,
    }

    let url = format!("https://crates.io/api/v1/crates/{CRATE}");
    let body = http_get(&url)?;
    let c: Crate = serde_json::from_str(&body).context("parse crates.io response")?;
    Ok(c.inner.newest_version)
}

/// Download and install the latest binary, replacing the running executable.
/// Strategy: if cargo is available → `cargo install klean`, else download
/// the pre-built binary from GitHub releases for this platform.
pub fn self_update(dry_run: bool) -> Result<()> {
    let info = VersionInfo::fetch()?;
    if !info.update_available {
        println!("✓ klean {} is already up to date.", info.current);
        return Ok(());
    }

    println!("⬆  Updating klean {} → {}…", info.current, info.latest);

    if dry_run {
        println!("(dry-run) no changes applied");
        return Ok(());
    }

    // Prefer cargo if available; it handles compilation + placement automatically.
    if which_cargo() {
        println!("  via cargo install klean…");
        let status = std::process::Command::new("cargo")
            .args(["install", "klean", "--locked", "--force"])
            .status()
            .context("run cargo install")?;
        if !status.success() {
            bail!("cargo install klean failed");
        }
        println!("✓ updated to {}", info.latest);
        return Ok(());
    }

    // Fallback: download pre-built binary from GitHub releases.
    download_release(&info.latest, dry_run)
}

fn download_release(version: &str, dry_run: bool) -> Result<()> {
    let triple = target_triple();
    let ext = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };
    let filename = format!("klean-v{version}-{triple}.{ext}");
    let url = format!("https://github.com/{REPO}/releases/download/v{version}/{filename}");

    println!("  downloading {filename}…");

    let body_bytes = http_get_bytes(&url).with_context(|| format!("download {url}"))?;

    if dry_run {
        println!(
            "(dry-run) downloaded {} bytes — no changes applied",
            body_bytes.len()
        );
        return Ok(());
    }

    let exe_path = current_exe_path()?;
    install_archive(&body_bytes, ext, &exe_path)?;

    println!("✓ klean {version} instalado em {}", exe_path.display());
    Ok(())
}

fn install_archive(data: &[u8], ext: &str, dest: &PathBuf) -> Result<()> {
    use std::io::Read;

    let tmp_dir = tempfile_dir(dest);
    std::fs::create_dir_all(&tmp_dir)?;

    let binary_name = if cfg!(target_os = "windows") {
        "klean.exe"
    } else {
        "klean"
    };

    // Write the raw archive to a temp file so we can extract from it.
    let archive_path = tmp_dir.join(format!("klean-download.{ext}"));
    std::fs::write(&archive_path, data)?;

    let binary_bytes: Vec<u8> = if ext == "tar.gz" {
        let file = std::fs::File::open(&archive_path)?;
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        let mut bytes = Vec::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            if path.file_name().and_then(|n| n.to_str()) == Some(binary_name) {
                entry.read_to_end(&mut bytes)?;
                break;
            }
        }
        let _ = std::fs::remove_file(&archive_path);
        if bytes.is_empty() {
            bail!("binary '{binary_name}' not found in archive");
        }
        bytes
    } else {
        // Windows .zip: extract with PowerShell (avoids the `zip` crate dep).
        let out_dir = tmp_dir.join("extracted");
        std::fs::create_dir_all(&out_dir)?;
        let status = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -Force -Path '{}' -DestinationPath '{}'",
                    archive_path.display(),
                    out_dir.display()
                ),
            ])
            .status()
            .context("run PowerShell Expand-Archive")?;
        let _ = std::fs::remove_file(&archive_path);
        if !status.success() {
            bail!("Expand-Archive failed");
        }
        let bin_path = out_dir.join(binary_name);
        if !bin_path.exists() {
            bail!("binary '{binary_name}' not found after extraction");
        }
        let bytes = std::fs::read(&bin_path)?;
        let _ = std::fs::remove_dir_all(&out_dir);
        bytes
    };

    // Write to a temp path first, then atomically replace.
    let tmp_bin = tmp_dir.join(binary_name);
    std::fs::write(&tmp_bin, &binary_bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_bin, std::fs::Permissions::from_mode(0o755))?;
    }

    // Rename current exe to .bak, put new one in place.
    let bak = dest.with_extension("bak");
    let _ = std::fs::rename(dest, &bak);
    std::fs::rename(&tmp_bin, dest).with_context(|| {
        // Restore backup if move fails.
        let _ = std::fs::rename(&bak, dest);
        format!("install new binary at {}", dest.display())
    })?;
    let _ = std::fs::remove_file(&bak);

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn which_cargo() -> bool {
    std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn current_exe_path() -> Result<PathBuf> {
    std::env::current_exe().context("locate current executable")
}

fn tempfile_dir(beside: &Path) -> PathBuf {
    beside
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"))
        .join(".klean-update-tmp")
}

fn target_triple() -> &'static str {
    // Match the asset names produced by build-release.yml
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-musl";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-unknown-linux-musl";
    #[cfg(target_os = "windows")]
    return "x86_64-pc-windows-msvc";
    #[allow(unreachable_code)]
    "unknown"
}

fn http_get(url: &str) -> Result<String> {
    let bytes = http_get_bytes(url)?;
    String::from_utf8(bytes).context("UTF-8 decode")
}

fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    // Try ureq first (no extra dep needed here — falls back to curl).
    // We use curl to avoid adding a heavy HTTP dep to the binary.
    let out = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-A",
            &format!("klean/{} (self-update)", env!("CARGO_PKG_VERSION")),
            url,
        ])
        .output()
        .context("run curl")?;

    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        bail!("curl failed for {url}: {msg}");
    }
    Ok(out.stdout)
}
