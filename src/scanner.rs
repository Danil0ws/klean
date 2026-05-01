use crate::ignore::IgnoreRules;
use crate::patterns::ArtifactPattern;
use anyhow::{Context, Result};
use humansize::format_size;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// These directories are skipped during traversal by default to avoid noise
// and risky/irrelevant scanning. If a directory name matches an artifact
// target, it is still reported as a result, but we never descend into it.
const GLOBAL_IGNORE: &[&str] = &[
    // Version controls
    ".git",
    ".svn",
    ".hg",
    ".fossil",
    // System folders
    ".Trash",
    ".Trashes",
    "System Volume Information",
    ".Spotlight-V100",
    ".fseventsd",
    // Tools and environment
    ".nvm",
    ".rvm",
    ".rustup",
    ".pyenv",
    ".rbenv",
    ".asdf",
    ".deno",
    ".local",
    // IDEs
    ".vscode",
    ".idea",
    ".vs",
    ".settings",
    // Other
    "snap",
    ".flatpak-info",
    // Heavy/common
    "node_modules",
    "__pycache__",
    "target",
    "build",
    "dist",
    ".cache",
    ".venv",
    "venv",
];

// Absolute path prefixes treated as sensitive by default.
// They are skipped unless explicitly enabled by CLI flag.
const SENSITIVE_SYSTEM_PREFIXES: &[&str] = &[
    "/System",
    "/Library",
    "/usr",
    "/bin",
    "/sbin",
    "/private",
    "/etc",
    "/var",
    "/Applications",
];

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Artifact {
    pub path: PathBuf,
    pub size: u64,
    pub name: String,
    pub pattern_name: String,
    pub modified: Option<std::time::SystemTime>,
    pub is_safe: bool,
}

impl Artifact {
    pub fn size_string(&self) -> String {
        format_size(self.size, humansize::BINARY)
    }
}

pub struct ArtifactScanner {
    root: PathBuf,
    ignore_rules: IgnoreRules,
    patterns: Vec<ArtifactPattern>,
    filter: Option<String>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    allow_system_paths: bool,
}

impl ArtifactScanner {
    pub fn new(root: PathBuf, ignore_rules: IgnoreRules, patterns: Vec<ArtifactPattern>) -> Self {
        ArtifactScanner {
            root,
            ignore_rules,
            patterns,
            filter: None,
            min_size: None,
            max_size: None,
            allow_system_paths: false,
        }
    }

    pub fn with_filter(mut self, filter: Option<String>) -> Self {
        self.filter = filter;
        self
    }

    pub fn with_size_limits(mut self, min_size: Option<u64>, max_size: Option<u64>) -> Self {
        self.min_size = min_size;
        self.max_size = max_size;
        self
    }

    pub fn with_allow_system_paths(mut self, allow_system_paths: bool) -> Self {
        self.allow_system_paths = allow_system_paths;
        self
    }

    /// Scan directory and find all artifacts
    pub fn scan(&self) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();
        let mut seen_paths: HashSet<PathBuf> = HashSet::new();
        let patterns = &self.patterns;

        let mut iter = WalkDir::new(&self.root).into_iter();

        while let Some(entry_res) = iter.next() {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let is_dir = entry.file_type().is_dir();

            // Skip sensitive system paths unless explicitly allowed.
            if is_dir && !self.allow_system_paths && self.is_sensitive_system_path(path) {
                iter.skip_current_dir();
                continue;
            }

            // GLOBAL_IGNORE behavior:
            // - Skip traversal for globally ignored dirs
            // - But if dir name is a target, allow it to be reported (still no descent later)
            if is_dir
                && self.is_globally_ignored_name(file_name)
                && !self.is_target_name(file_name)
            {
                iter.skip_current_dir();
                continue;
            }

            // Check if ignored
            if self.ignore_rules.is_ignored(path) {
                // If this is a directory and ignored, skip its contents
                // But if the directory name is a target, still allow it as a result.
                if is_dir && !self.is_target_name(file_name) {
                    iter.skip_current_dir();
                    continue;
                }
            }

            // Check if matches any pattern
            for pattern in patterns {
                // Apply filter if specified
                if let Some(ref filter) = self.filter {
                    if !pattern.name.contains(filter)
                        && !pattern.patterns.iter().any(|p| p.contains(filter))
                    {
                        continue;
                    }
                }

                for p in &pattern.patterns {
                    if self.path_matches_pattern(path, p) {
                        // Only consider directories as artifacts
                        if let Ok(metadata) = entry.metadata() {
                            if !metadata.is_dir() {
                                continue;
                            }

                            let size = calculate_dir_size(path);

                            // Apply size filters
                            if let Some(min) = self.min_size {
                                if size < min {
                                    continue;
                                }
                            }
                            if let Some(max) = self.max_size {
                                if size > max {
                                    continue;
                                }
                            }

                            artifacts.push(Artifact {
                                path: path.to_path_buf(),
                                size,
                                name: p.clone(),
                                pattern_name: pattern.name.clone(),
                                modified: entry.metadata().ok().and_then(|m| m.modified().ok()),
                                is_safe: pattern.safe_to_delete,
                            });

                            // Prevent duplicates when multiple patterns point to the same path.
                            if !seen_paths.insert(path.to_path_buf()) {
                                artifacts.pop();
                            }

                            // Skip descending into this directory so we don't list nested
                            // artifacts (e.g., node_modules/zod) as separate items.
                            iter.skip_current_dir();
                        }
                    }
                }
            }
        }

        // Always present bigger artifacts first.
        artifacts.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));

        Ok(artifacts)
    }

    fn is_target_name(&self, name: &str) -> bool {
        self.patterns
            .iter()
            .flat_map(|p| p.patterns.iter())
            .any(|p| p == name)
    }

    fn is_globally_ignored_name(&self, name: &str) -> bool {
        GLOBAL_IGNORE.contains(&name)
    }

    fn is_sensitive_system_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        SENSITIVE_SYSTEM_PREFIXES
            .iter()
            .any(|prefix| path_str == *prefix || path_str.starts_with(&format!("{}/", prefix)))
    }

    fn path_matches_pattern(&self, path: &Path, pattern: &str) -> bool {
        if let Some(file_name) = path.file_name() {
            if let Some(name) = file_name.to_str() {
                // Match only when the path's final component equals the pattern
                // (e.g. a directory literally named "node_modules").
                // Avoid matching any path that merely contains the pattern string
                // to prevent listing nested inner paths inside the artifact
                // (like node_modules/zod/src) as separate artifacts.
                return name == pattern;
            }
        }
        false
    }
}

/// Calculate total size of a directory recursively
pub fn calculate_dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// Parse size string like "100MB", "1GB" to bytes
pub fn parse_size(size_str: &str) -> Result<u64> {
    let upper = size_str.to_uppercase();
    let size_str = upper.trim();

    let (num_str, unit) = if size_str.ends_with("GB") {
        (&size_str[..size_str.len() - 2], 1024u64 * 1024 * 1024)
    } else if size_str.ends_with("MB") {
        (&size_str[..size_str.len() - 2], 1024u64 * 1024)
    } else if size_str.ends_with("KB") {
        (&size_str[..size_str.len() - 2], 1024u64)
    } else if size_str.ends_with("B") {
        (&size_str[..size_str.len() - 1], 1u64)
    } else {
        // Try parsing as plain number (bytes)
        (size_str, 1u64)
    };

    let num: u64 = num_str
        .parse()
        .context(format!("Invalid size format: {}", size_str))?;

    Ok(num * unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100MB").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_size("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("512KB").unwrap(), 512 * 1024);
        assert_eq!(parse_size("1024B").unwrap(), 1024);
    }

    #[test]
    fn test_artifact_size_string() {
        let artifact = Artifact {
            path: PathBuf::from("/tmp/test"),
            size: 1024 * 1024,
            name: "test".to_string(),
            pattern_name: "test_pattern".to_string(),
            modified: None,
            is_safe: true,
        };
        assert!(artifact.size_string().contains("MiB"));
    }
}
