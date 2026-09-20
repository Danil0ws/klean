use crate::patterns::{name_matches, ArtifactPattern};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Per-project deletion policy.
///
/// ```toml
/// [[projects]]
/// path = "~/Dev/voz-amiga"        # absolute, `~`-prefixed, or glob
/// enabled = true                   # false = never clean anything here
/// allow = ["node_modules", ".next"] # if set, only these may be deleted
/// deny = ["target"]                # never deleted here
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProjectRule {
    /// Project root: absolute path, `~`-prefixed path, or glob (`*`, `?`).
    pub path: String,

    /// `false` disables deletion for the whole project (default: true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// If present and non-empty, only these artifacts may be deleted.
    /// Matched against the directory name and the pattern name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,

    /// Artifacts that may never be deleted in this project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,
}

impl ProjectRule {
    /// Length of the matched path, or `None` when the rule doesn't apply to this
    /// project. The longest match wins, so nested rules override broader ones.
    pub fn matched_len(&self, project: &Path) -> Option<usize> {
        let pattern = expand_tilde(&self.path);
        if pattern.is_empty() {
            return None;
        }

        let project_str = project.to_string_lossy();

        if pattern.contains('*') || pattern.contains('?') {
            return name_matches(&project_str, &pattern).then_some(pattern.len());
        }

        let trimmed = pattern.trim_end_matches('/');
        if project_str == trimmed || project_str.starts_with(&format!("{}/", trimmed)) {
            return Some(trimmed.len());
        }

        None
    }
}

/// Expand a leading `~/` to the user's home directory.
pub fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{}", home.to_string_lossy().trim_end_matches('/'), rest);
        }
    }
    path.to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KleanConfig {
    /// Additional artifact patterns to include
    pub patterns: Option<Vec<ArtifactPattern>>,

    /// Directory to move artifacts to instead of deleting
    pub backup_dir: Option<PathBuf>,

    /// Respect .gitignore patterns
    pub respect_gitignore: Option<bool>,

    /// Default verbosity level
    pub verbosity: Option<u8>,

    /// Additional patterns as simple strings
    pub custom_patterns: Option<Vec<String>>,

    /// Per-project deletion policy (`[[projects]]` entries)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects: Option<Vec<ProjectRule>>,
}

impl Default for KleanConfig {
    fn default() -> Self {
        KleanConfig {
            patterns: None,
            backup_dir: None,
            respect_gitignore: Some(true),
            verbosity: None,
            custom_patterns: None,
            projects: None,
        }
    }
}

impl KleanConfig {
    /// Load config from local klean.toml
    pub fn from_local(root: &Path) -> Result<Option<Self>> {
        let config_path = root.join("klean.toml");
        if !config_path.exists() {
            return Ok(None);
        }
        Self::from_file(&config_path)
    }

    /// Load config from global ~/.config/klean/config.toml
    pub fn from_global() -> Result<Option<Self>> {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("klean").join("config.toml");
            if config_path.exists() {
                return Self::from_file(&config_path);
            }
        }
        Ok(None)
    }

    /// Load config from specific file
    pub fn from_file(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }

        let content =
            fs::read_to_string(path).context(format!("Failed to read config file: {:?}", path))?;

        let config: KleanConfig =
            toml::from_str(&content).context(format!("Failed to parse config file: {:?}", path))?;

        Ok(Some(config))
    }

    /// Merge with another config (other takes precedence)
    pub fn merge(&self, other: &KleanConfig) -> KleanConfig {
        // Project rules accumulate (global first, then local) so a repo can add
        // rules on top of the global ones; the most specific path wins.
        let mut projects = self.projects.clone().unwrap_or_default();
        if let Some(extra) = &other.projects {
            projects.extend(extra.clone());
        }

        KleanConfig {
            patterns: other.patterns.clone().or_else(|| self.patterns.clone()),
            backup_dir: other.backup_dir.clone().or_else(|| self.backup_dir.clone()),
            respect_gitignore: other.respect_gitignore.or(self.respect_gitignore),
            verbosity: other.verbosity.or(self.verbosity),
            custom_patterns: other
                .custom_patterns
                .clone()
                .or_else(|| self.custom_patterns.clone()),
            projects: if projects.is_empty() {
                None
            } else {
                Some(projects)
            },
        }
    }

    /// Save config to file
    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("Invalid path")?;
        fs::create_dir_all(parent)
            .context(format!("Failed to create config directory: {:?}", parent))?;

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(path, content).context(format!("Failed to write config file: {:?}", path))?;

        Ok(())
    }
}

/// Helper module for directory paths
pub mod dirs {
    use std::path::PathBuf;

    pub fn config_dir() -> Option<PathBuf> {
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            Some(PathBuf::from(config_home))
        } else if let Ok(home) = std::env::var("HOME") {
            Some(PathBuf::from(home).join(".config"))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = KleanConfig::default();
        assert_eq!(config.respect_gitignore, Some(true));
    }

    #[test]
    fn test_config_merge() {
        let config1 = KleanConfig {
            backup_dir: Some(PathBuf::from("/tmp/backup")),
            verbosity: Some(1),
            ..Default::default()
        };

        let config2 = KleanConfig {
            verbosity: Some(2),
            ..Default::default()
        };

        let merged = config1.merge(&config2);
        assert_eq!(merged.backup_dir, Some(PathBuf::from("/tmp/backup")));
        assert_eq!(merged.verbosity, Some(2));
    }
}
