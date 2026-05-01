use anyhow::{Context, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::fs;
use std::path::{Path, PathBuf};

/// Manages ignore rules from .klignore and optionally .gitignore files
pub struct IgnoreRules {
    gitignore: Option<Gitignore>,
    respect_gitignore: bool,
}

impl IgnoreRules {
    /// Create a new IgnoreRules instance, loading rules from the given directory
    pub fn from_path(root: &Path, respect_gitignore: bool) -> Result<Self> {
        let mut gitignore = None;

        // Load .klignore first (takes precedence)
        if let Some(loaded) = Self::load_klignore(root)? {
            gitignore = Some(loaded);
        } else if respect_gitignore {
            // Fall back to .gitignore if no .klignore
            if let Some(loaded) = Self::load_gitignore(root)? {
                gitignore = Some(loaded);
            }
        }

        Ok(IgnoreRules {
            gitignore,
            respect_gitignore,
        })
    }

    /// Load .klignore file and convert it to gitignore format
    fn load_klignore(root: &Path) -> Result<Option<Gitignore>> {
        let klignore_path = root.join(".klignore");
        if !klignore_path.exists() {
            return Ok(None);
        }

        Self::build_gitignore_from_file(&klignore_path)
    }

    /// Load .gitignore file
    fn load_gitignore(root: &Path) -> Result<Option<Gitignore>> {
        let gitignore_path = root.join(".gitignore");
        if !gitignore_path.exists() {
            return Ok(None);
        }

        Self::build_gitignore_from_file(&gitignore_path)
    }

    /// Build a Gitignore object from a file
    fn build_gitignore_from_file(path: &Path) -> Result<Option<Gitignore>> {
        let content = fs::read_to_string(path).context(format!("Failed to read {:?}", path))?;

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut builder = GitignoreBuilder::new(parent);

        for line in content.lines() {
            let line = line.trim();
            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            builder
                .add_line(None, line)
                .context(format!("Failed to add ignore pattern: {}", line))?;
        }

        Ok(builder.build().ok())
    }

    /// Check if a path should be ignored
    pub fn is_ignored(&self, path: &Path) -> bool {
        if let Some(ref gi) = self.gitignore {
            // The gitignore matcher returns a MatchResult
            gi.matched(path, false).is_ignore()
        } else {
            false
        }
    }

    /// Load rules from a custom klignore file
    pub fn from_custom_file(path: &Path) -> Result<Self> {
        let gitignore = Self::build_gitignore_from_file(path)?;
        Ok(IgnoreRules {
            gitignore,
            respect_gitignore: true,
        })
    }
}

/// Parse .klignore patterns manually (simpler approach without regex)
pub struct KlignoreParser;

impl KlignoreParser {
    /// Parse klignore patterns from file content
    pub fn parse(content: &str) -> Vec<String> {
        content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                // Handle negation patterns
                if line.starts_with('!') {
                    line.to_string()
                } else {
                    line.to_string()
                }
            })
            .collect()
    }

    /// Check if a path matches any pattern (simplified glob matching)
    pub fn matches_pattern(path: &Path, pattern: &str) -> bool {
        if pattern.is_empty() {
            return false;
        }

        let path_str = path.to_string_lossy();

        // Handle directory-only patterns (ending with /)
        let is_dir_pattern = pattern.ends_with('/');
        let pattern = if is_dir_pattern {
            &pattern[..pattern.len() - 1]
        } else {
            pattern
        };

        // Handle negation
        if pattern.starts_with('!') {
            return false; // Already handled elsewhere
        }

        // Simple pattern matching
        if pattern.contains('*') {
            // Wildcard matching
            wildcard_match(&path_str, pattern)
        } else if pattern.starts_with('/') {
            // Absolute path from root
            path_str == &pattern[1..]
        } else {
            // Relative path - check if it appears anywhere
            path_str.contains(pattern)
                || path_str.ends_with(&format!("/{}", pattern))
                || path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == pattern)
                    .unwrap_or(false)
        }
    }
}

/// Simple wildcard matching
fn wildcard_match(text: &str, pattern: &str) -> bool {
    let mut text_chars = text.chars().peekable();
    let mut pattern_chars = pattern.chars().peekable();

    while let Some(&p) = pattern_chars.peek() {
        match p {
            '*' => {
                pattern_chars.next();
                if pattern_chars.peek().is_none() {
                    return true; // * at end matches everything
                }
                // Find next match
                loop {
                    if wildcard_match(
                        &text_chars.clone().collect::<String>(),
                        &pattern_chars.clone().collect::<String>(),
                    ) {
                        return true;
                    }
                    if text_chars.next().is_none() {
                        return false;
                    }
                }
            }
            '?' => {
                pattern_chars.next();
                if text_chars.next().is_none() {
                    return false;
                }
            }
            c => {
                pattern_chars.next();
                if text_chars.next() != Some(c) {
                    return false;
                }
            }
        }
    }

    text_chars.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_klignore() {
        let content = r#"
# This is a comment
node_modules
.venv

# Protected directories
/important-dir
"#;
        let patterns = KlignoreParser::parse(content);
        assert_eq!(patterns.len(), 3);
        assert!(patterns.contains(&"node_modules".to_string()));
    }

    #[test]
    fn test_wildcard_match() {
        assert!(wildcard_match("test.txt", "*.txt"));
        assert!(wildcard_match("test", "te?t"));
        assert!(wildcard_match("anything", "*"));
        assert!(!wildcard_match("test.rs", "*.txt"));
    }

    #[test]
    fn test_pattern_matching() {
        let path = Path::new("src/node_modules");
        assert!(KlignoreParser::matches_pattern(path, "node_modules"));
        assert!(KlignoreParser::matches_pattern(path, "*"));
    }
}
