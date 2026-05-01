use crate::ignore::IgnoreRules;
use crate::patterns::ArtifactPattern;
use anyhow::{Context, Result};
use humansize::format_size;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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

    pub fn relative_path(&self, base: &Path) -> PathBuf {
        self.path
            .strip_prefix(base)
            .unwrap_or(&self.path)
            .to_path_buf()
    }
}

pub struct ArtifactScanner {
    root: PathBuf,
    ignore_rules: IgnoreRules,
    patterns: Vec<ArtifactPattern>,
    filter: Option<String>,
    min_size: Option<u64>,
    max_size: Option<u64>,
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

    /// Scan directory and find all artifacts
    pub fn scan(&self) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();
        let patterns = &self.patterns;

        for entry in WalkDir::new(&self.root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            // Check if ignored
            if self.ignore_rules.is_ignored(path) {
                continue;
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
                        }
                    }
                }
            }
        }

        Ok(artifacts)
    }

    fn path_matches_pattern(&self, path: &Path, pattern: &str) -> bool {
        if let Some(file_name) = path.file_name() {
            if let Some(name) = file_name.to_str() {
                return name == pattern
                    || path.to_string_lossy().contains(&format!("/{}", pattern));
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
