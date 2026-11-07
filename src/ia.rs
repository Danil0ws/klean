//! AI environment scanner and cleaner.

use anyhow::{bail, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AIType {
    Skill,
    CLI,
    MCPServer,
}

impl AIType {
    pub fn label(&self) -> &'static str {
        match self {
            AIType::Skill => "skill",
            AIType::CLI => "cli",
            AIType::MCPServer => "mcp-server",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            AIType::Skill => "🎯",
            AIType::CLI => "⚙️",
            AIType::MCPServer => "🔌",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AIItem {
    pub name: String,
    pub version: String,
    pub item_type: AIType,
    pub path: PathBuf,
    /// Disk usage in bytes (0 when unknown).
    pub size_bytes: u64,
    /// True if this is identified as a duplicate.
    pub is_duplicate: bool,
    /// Optional description from metadata.
    pub description: Option<String>,
    /// Content hash (for duplicate detection by content).
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConsolidationReport {
    pub moved: usize,
    pub symlinks_created: usize,
    pub duplicates_removed: usize,
    pub space_freed: u64,
}

impl AIItem {
    pub fn size_string(&self) -> String {
        if self.size_bytes == 0 {
            return "—".to_string();
        }
        humansize::format_size(self.size_bytes, humansize::BINARY)
    }
}

/// Scan for all AI-related items.
pub fn scan_all() -> Vec<AIItem> {
    let mut items = Vec::new();
    items.extend(scan_skills());
    items.extend(scan_ai_clis());
    items.extend(scan_mcp_servers());
    mark_duplicates(&mut items);
    items
}

/// Remove an AI item (noop when dry_run is true).
pub fn remove_item(item: &AIItem, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }

    match item.item_type {
        AIType::Skill => {
            if item.path.is_file() {
                std::fs::remove_file(&item.path)?;
            } else if item.path.is_dir() {
                std::fs::remove_dir_all(&item.path)?;
            }
            Ok(())
        }
        AIType::CLI => {
            // Uninstall via package manager if possible
            if let Some(manager) = detect_package_manager(&item.name) {
                match manager.as_str() {
                    "cargo" => run("cargo", &["uninstall", &item.name]),
                    "npm" => run("npm", &["uninstall", "-g", &item.name]),
                    "pip" => run("pip3", &["uninstall", "-y", &item.name])
                        .or_else(|_| run("pip", &["uninstall", "-y", &item.name])),
                    "brew" => run("brew", &["uninstall", &item.name]),
                    _ => {
                        // Manual binary removal
                        if item.path.exists() {
                            std::fs::remove_file(&item.path)?;
                        }
                        Ok(())
                    }
                }
            } else if item.path.exists() {
                std::fs::remove_file(&item.path)?;
                Ok(())
            } else {
                bail!("cannot remove {}: path not found", item.name)
            }
        }
        AIType::MCPServer => {
            if item.path.is_file() {
                std::fs::remove_file(&item.path)?;
            } else if item.path.is_dir() {
                std::fs::remove_dir_all(&item.path)?;
            }
            Ok(())
        }
    }
}

// ── skills ────────────────────────────────────────────────────────────────────

fn scan_skills() -> Vec<AIItem> {
    let mut items = Vec::new();
    let home = match home_dir() {
        Some(h) => h,
        None => return items,
    };

    // Global skill directories across supported AI tools. Shared/alias paths are
    // deduplicated by canonical location so the same skill is not scanned twice.
    let mut skill_dirs: Vec<PathBuf> = vec![
        home.join(".claude").join("skills"),
        home.join(".agents").join("skills"),
        home.join(".ia-agents"),
        home.join(".cursor").join("skills"),
        home.join(".copilot").join("skills"),
        home.join(".codex").join("skills"),
        home.join(".gemini").join("skills"),
        home.join(".hermes").join("skills"),
        home.join(".config").join("opencode").join("skills"),
        home.join(".config").join("goose").join("skills"),
        home.join(".cline").join("skills"),
        home.join(".config").join("agents").join("skills"),
        home.join(".config").join("amp").join("skills"),
        home.join(".warp").join("skills"),
    ];

    // Project-level skill directories in the current working directory.
    if let Ok(cwd) = std::env::current_dir() {
        for rel in [
            ".agents/skills",
            ".claude/skills",
            ".opencode/skills",
            ".cursor/skills",
            ".cline/skills",
            ".goose/skills",
            ".warp/skills",
        ] {
            skill_dirs.push(cwd.join(rel));
        }
    }

    let mut seen_dirs: HashSet<PathBuf> = HashSet::new();
    for dir in skill_dirs {
        if !dir.exists() {
            continue;
        }
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if !seen_dirs.insert(canonical) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if !n.starts_with('.') => n.to_string(),
                _ => continue,
            };

            let size_bytes = if path.is_dir() {
                dir_size(&path)
            } else {
                path.metadata().map(|m| m.len()).unwrap_or(0)
            };

            let (description, version) = parse_skill_metadata(&path);
            let content_hash = compute_content_hash(&path);

            items.push(AIItem {
                name,
                version,
                item_type: AIType::Skill,
                path,
                size_bytes,
                is_duplicate: false,
                description,
                content_hash,
            });
        }
    }

    items
}

fn parse_skill_metadata(path: &Path) -> (Option<String>, String) {
    // Try to read frontmatter from .md files
    let read_path = if path.is_dir() {
        path.join("README.md")
    } else {
        path.to_path_buf()
    };

    if !read_path.exists() || !read_path.extension().map(|e| e == "md").unwrap_or(false) {
        return (None, "?".to_string());
    }

    let Ok(content) = std::fs::read_to_string(&read_path) else {
        return (None, "?".to_string());
    };

    let mut description = None;
    let mut version = "?".to_string();

    if content.starts_with("---") {
        let lines: Vec<&str> = content.lines().skip(1).collect();
        for line in lines {
            if line.trim() == "---" {
                break;
            }
            if let Some(desc) = line.strip_prefix("description:") {
                description = Some(desc.trim().to_string());
            }
            if let Some(ver) = line.strip_prefix("version:") {
                version = ver.trim().to_string();
            }
        }
    }

    (description, version)
}

// ── AI CLIs ───────────────────────────────────────────────────────────────────

fn scan_ai_clis() -> Vec<AIItem> {
    let mut items = Vec::new();
    let home = match home_dir() {
        Some(h) => h,
        None => return items,
    };

    let scan_dirs = vec![
        home.join(".local").join("bin"),
        home.join("bin"),
        home.join(".cargo").join("bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];

    let ai_patterns = vec![
        "ollama",
        "llm",
        "mcp-",
        "claude",
        "browse",
        "rtk",
        "codex",
        "cursor",
        "aider",
        "gpt",
        "chatgpt",
        "anthropic",
        "openai",
        "gemini",
        "perplexity",
        "hermes",
        "goose",
        "cline",
        "kimi",
        "dexto",
        "warp",
        "replit",
        "amp",
        "opencode",
        "copilot",
    ];

    let mut seen = HashSet::new();

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

            // Skip if not AI-related
            let is_ai_related = ai_patterns.iter().any(|p| name.to_lowercase().contains(p));
            if !is_ai_related {
                continue;
            }

            if !is_executable(&path) {
                continue;
            }

            // Deduplicate by canonical path
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !seen.insert(canonical) {
                continue;
            }

            let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
            let version = get_binary_version(&path);

            items.push(AIItem {
                name,
                version,
                item_type: AIType::CLI,
                path,
                size_bytes,
                is_duplicate: false,
                description: None,
                content_hash: None,
            });
        }
    }

    items
}

fn detect_package_manager(name: &str) -> Option<String> {
    // Check if binary was installed via cargo
    let cargo_bin = home_dir().map(|h| h.join(".cargo").join("bin").join(name));
    if let Some(path) = cargo_bin {
        if path.exists() {
            return Some("cargo".to_string());
        }
    }

    // Check npm global
    if let Ok(out) = Command::new("npm").args(["list", "-g", name]).output() {
        if out.status.success() {
            return Some("npm".to_string());
        }
    }

    // Check pip
    if let Ok(out) = Command::new("pip3").args(["show", name]).output() {
        if out.status.success() {
            return Some("pip".to_string());
        }
    }

    // Check brew
    if let Ok(out) = Command::new("brew").args(["list", name]).output() {
        if out.status.success() {
            return Some("brew".to_string());
        }
    }

    None
}

// ── MCP servers ───────────────────────────────────────────────────────────────

fn scan_mcp_servers() -> Vec<AIItem> {
    let mut items = Vec::new();
    let home = match home_dir() {
        Some(h) => h,
        None => return items,
    };

    let mcp_dirs = vec![
        home.join(".claude").join("mcp-servers"),
        home.join(".config").join("claude").join("mcp"),
        home.join(".config").join("opencode"),
        home.join(".cursor").join("mcp"),
        home.join(".copilot").join("mcp"),
        home.join(".config").join("goose").join("mcp"),
    ];

    for dir in mcp_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if !n.starts_with('.') => n.to_string(),
                _ => continue,
            };

            let size_bytes = if path.is_dir() {
                dir_size(&path)
            } else {
                path.metadata().map(|m| m.len()).unwrap_or(0)
            };

            items.push(AIItem {
                name,
                version: "?".to_string(),
                item_type: AIType::MCPServer,
                path,
                size_bytes,
                is_duplicate: false,
                description: None,
                content_hash: None,
            });
        }
    }

    items
}

// ── duplicate detection ───────────────────────────────────────────────────────

fn mark_duplicates(items: &mut [AIItem]) {
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    let mut hash_counts: HashMap<String, usize> = HashMap::new();
    for item in items.iter() {
        *name_counts.entry(item.name.clone()).or_insert(0) += 1;
        if let Some(h) = &item.content_hash {
            *hash_counts.entry(h.clone()).or_insert(0) += 1;
        }
    }

    for item in items.iter_mut() {
        let by_name = name_counts.get(&item.name).copied().unwrap_or(0) > 1;
        let by_hash = item
            .content_hash
            .as_ref()
            .and_then(|h| hash_counts.get(h).copied())
            .unwrap_or(0)
            > 1;
        if by_name || by_hash {
            item.is_duplicate = true;
        }
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
    for flag in &["--version", "-V", "-v"] {
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

fn compute_content_hash(path: &Path) -> Option<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut files: Vec<PathBuf> = Vec::new();
    if path.is_file() {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.is_file() {
                    out.push(p);
                }
            }
        }
        walk(path, &mut files);
    } else {
        return None;
    }

    if files.is_empty() {
        return None;
    }

    files.sort();
    let mut hasher = DefaultHasher::new();
    for f in files {
        if let Ok(rel) = f.strip_prefix(path) {
            rel.to_string_lossy().hash(&mut hasher);
        }
        if let Ok(bytes) = std::fs::read(&f) {
            bytes.hash(&mut hasher);
        }
    }
    Some(format!("{:x}", hasher.finish()))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}
