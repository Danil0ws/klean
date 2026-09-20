//! Pattern plugins: extra artifact types without recompiling klean.
//!
//! A plugin is a `.toml` file in a `plugins/` directory containing either a
//! single `ArtifactPattern` table or a `[[patterns]]` array of them:
//!
//! ```toml
//! # ~/.config/klean/plugins/blender.toml  (global)
//! # ./.klean/plugins/blender.toml         (per project)
//! [[patterns]]
//! name = "blender-cache"
//! patterns = ["blender_cache", "*.blend1"]
//! languages = ["Generic"]
//! description = "Blender temporary render cache"
//! safe_to_delete = true
//! ```

use crate::config::dirs;
use crate::patterns::ArtifactPattern;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(untagged)]
enum PluginFile {
    Multiple { patterns: Vec<ArtifactPattern> },
    Single(ArtifactPattern),
}

pub struct Plugin {
    pub source: PathBuf,
    pub patterns: Vec<ArtifactPattern>,
}

/// Directories searched for plugins: global first, then the scan root.
pub fn plugin_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs_found = Vec::new();
    if let Some(config) = dirs::config_dir() {
        dirs_found.push(config.join("klean").join("plugins"));
    }
    dirs_found.push(root.join(".klean").join("plugins"));
    dirs_found
}

/// Load every plugin that parses. A broken file is reported and skipped: one
/// bad plugin must not stop a scan.
pub fn load(root: &Path) -> Vec<Plugin> {
    let mut plugins = Vec::new();

    for dir in plugin_dirs(root) {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        files.sort();

        for file in files {
            let text = match fs::read_to_string(&file) {
                Ok(text) => text,
                Err(err) => {
                    eprintln!("⚠️  plugin {}: {}", file.display(), err);
                    continue;
                }
            };

            match toml::from_str::<PluginFile>(&text) {
                Ok(PluginFile::Multiple { patterns }) => plugins.push(Plugin {
                    source: file,
                    patterns,
                }),
                Ok(PluginFile::Single(pattern)) => plugins.push(Plugin {
                    source: file,
                    patterns: vec![pattern],
                }),
                Err(err) => eprintln!("⚠️  plugin {} ignorado: {}", file.display(), err),
            }
        }
    }

    plugins
}

pub fn patterns(root: &Path) -> Vec<ArtifactPattern> {
    load(root)
        .into_iter()
        .flat_map(|plugin| plugin.patterns)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_single_and_multi_pattern_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let dir = root.join(".klean/plugins");
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("one.toml"),
            r#"
name = "blender-cache"
patterns = ["blender_cache"]
languages = ["Generic"]
description = "Blender cache"
safe_to_delete = true
"#,
        )
        .unwrap();
        fs::write(
            dir.join("two.toml"),
            r#"
[[patterns]]
name = "gradle-cache"
patterns = [".gradle"]
languages = ["Java"]
description = "Gradle cache"
safe_to_delete = true
"#,
        )
        .unwrap();
        fs::write(dir.join("broken.toml"), "name = ").unwrap();

        let loaded = patterns(root);
        let names: Vec<&str> = loaded.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"blender-cache"), "{names:?}");
        assert!(names.contains(&"gradle-cache"), "{names:?}");
        assert_eq!(loaded.len(), 2, "broken plugin must be skipped: {names:?}");
    }
}
