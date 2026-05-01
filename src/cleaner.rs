use crate::scanner::Artifact;
use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};

pub enum CleanerAction {
    Delete,
    Backup,
}

pub struct Cleaner {
    action: CleanerAction,
    backup_dir: Option<PathBuf>,
}

impl Cleaner {
    pub fn new(action: CleanerAction, backup_dir: Option<PathBuf>) -> Self {
        Cleaner { action, backup_dir }
    }

    /// Check if an artifact is safe to delete (has marker files)
    pub fn is_safe_to_delete(&self, artifact: &Artifact) -> bool {
        if !artifact.is_safe {
            return false;
        }

        // Check for known marker files that indicate a regenerable directory
        let parent = artifact.path.parent().unwrap_or_else(|| Path::new("."));
        let markers = [
            "package.json",     // Node.js
            "Cargo.toml",       // Rust
            "pom.xml",          // Maven
            "build.gradle",     // Gradle
            ".csproj",          // C#
            "go.mod",           // Go
            "requirements.txt", // Python
            "setup.py",         // Python
            "Gemfile",          // Ruby
        ];

        markers.iter().any(|marker| parent.join(marker).exists())
    }

    /// Verify if it's safe to proceed with cleaning
    pub fn verify_safety(&self, artifacts: &[Artifact]) -> Result<()> {
        // Check if we're about to delete too much
        let total_size: u64 = artifacts.iter().map(|a| a.size).sum();

        if total_size > 50 * 1024 * 1024 * 1024 {
            // More than 50GB
            return Err(anyhow!(
                "Attempting to delete more than 50GB - please confirm manually"
            ));
        }

        // Warn about unsafe deletions
        let unsafe_count = artifacts.iter().filter(|a| !a.is_safe).count();
        if unsafe_count > 0 {
            eprintln!(
                "⚠️  Warning: {} artifacts marked as potentially unsafe",
                unsafe_count
            );
        }

        Ok(())
    }

    /// Clean artifacts (either delete or backup)
    pub fn clean(&self, artifacts: Vec<Artifact>, dry_run: bool) -> Result<CleanResult> {
        let total_items = artifacts.len();
        let pb = ProgressBar::new(total_items as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .context("Failed to set progress style")?,
        );

        let mut result = CleanResult {
            deleted: 0,
            backed_up: 0,
            failed: 0,
            total_size_freed: 0,
            errors: Vec::new(),
        };

        for artifact in artifacts {
            if dry_run {
                result.deleted += 1;
                result.total_size_freed += artifact.size;
                pb.inc(1);
                pb.set_message(format!("[DRY RUN] {}", artifact.path.display()));
                continue;
            }

            match &self.action {
                CleanerAction::Delete => match self.delete_artifact(&artifact) {
                    Ok(_) => {
                        result.deleted += 1;
                        result.total_size_freed += artifact.size;
                    }
                    Err(e) => {
                        result.failed += 1;
                        result
                            .errors
                            .push(format!("{}: {}", artifact.path.display(), e));
                    }
                },
                CleanerAction::Backup => {
                    if let Some(backup) = &self.backup_dir {
                        match self.backup_artifact(&artifact, backup) {
                            Ok(_) => {
                                result.backed_up += 1;
                                result.total_size_freed += artifact.size;
                            }
                            Err(e) => {
                                result.failed += 1;
                                result
                                    .errors
                                    .push(format!("{}: {}", artifact.path.display(), e));
                            }
                        }
                    }
                }
            }

            pb.inc(1);
            pb.set_message(artifact.name.clone());
        }

        pb.finish_with_message("✓ Cleaning complete!");

        Ok(result)
    }

    /// Delete an artifact permanently
    fn delete_artifact(&self, artifact: &Artifact) -> Result<()> {
        if artifact.path.is_dir() {
            fs::remove_dir_all(&artifact.path)
                .context(format!("Failed to delete directory: {:?}", artifact.path))
        } else {
            fs::remove_file(&artifact.path)
                .context(format!("Failed to delete file: {:?}", artifact.path))
        }
    }

    /// Backup an artifact by moving it
    fn backup_artifact(&self, artifact: &Artifact, backup_dir: &Path) -> Result<()> {
        fs::create_dir_all(backup_dir).context(format!(
            "Failed to create backup directory: {:?}",
            backup_dir
        ))?;

        let backup_name = artifact.path.file_name().context("Invalid artifact path")?;
        let backup_path = backup_dir.join(backup_name);

        // Handle naming conflicts
        let final_backup_path = self.find_unique_path(&backup_path)?;

        std::fs::rename(&artifact.path, &final_backup_path).context(format!(
            "Failed to move {:?} to {:?}",
            artifact.path, final_backup_path
        ))
    }

    /// Find a unique path for backup (avoid overwriting)
    fn find_unique_path(&self, path: &Path) -> Result<PathBuf> {
        if !path.exists() {
            return Ok(path.to_path_buf());
        }

        let parent = path.parent().context("Invalid path")?;
        let stem = path.file_stem().context("Invalid path")?;
        let ext = path.extension();

        let mut counter = 1;
        loop {
            let new_name = if let Some(ext) = ext {
                format!(
                    "{}_{}.{}",
                    stem.to_string_lossy(),
                    counter,
                    ext.to_string_lossy()
                )
            } else {
                format!("{}_{}", stem.to_string_lossy(), counter)
            };

            let new_path = parent.join(&new_name);
            if !new_path.exists() {
                return Ok(new_path);
            }
            counter += 1;
        }
    }
}

pub struct CleanResult {
    pub deleted: usize,
    pub backed_up: usize,
    pub failed: usize,
    pub total_size_freed: u64,
    pub errors: Vec<String>,
}

impl CleanResult {
    pub fn print_summary(&self) {
        println!("\n╔════════════════════════════════════╗");
        println!("║          CLEANING SUMMARY          ║");
        println!("╠════════════════════════════════════╣");
        println!("║ Deleted:       {:>18} ║", self.deleted);
        println!("║ Backed up:     {:>18} ║", self.backed_up);
        println!("║ Failed:        {:>18} ║", self.failed);
        println!(
            "║ Size freed:    {:>18} ║",
            humansize::format_size(self.total_size_freed, humansize::BINARY)
        );
        println!("╚════════════════════════════════════╝");

        if !self.errors.is_empty() {
            println!("\n⚠️  Errors encountered:");
            for error in &self.errors {
                println!("  • {}", error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_unique_path() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.txt");

        // Create initial file
        fs::write(&path, "test").unwrap();

        let cleaner = Cleaner::new(CleanerAction::Delete, None);
        let unique_path = cleaner.find_unique_path(&path).unwrap();

        assert_ne!(unique_path, path);
        assert!(unique_path.to_string_lossy().contains("_1"));
    }

    #[test]
    fn test_delete_artifact() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_file");
        fs::create_dir(&test_file).unwrap();

        let artifact = Artifact {
            path: test_file.clone(),
            size: 1024,
            name: "test".to_string(),
            pattern_name: "test".to_string(),
            modified: None,
            is_safe: true,
        };

        let cleaner = Cleaner::new(CleanerAction::Delete, None);
        assert!(cleaner.delete_artifact(&artifact).is_ok());
        assert!(!test_file.exists());
    }
}
