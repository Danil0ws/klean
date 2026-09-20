use crate::config::ProjectRule;
use crate::ignore::IgnoreRules;
use crate::patterns::{name_matches, ArtifactPattern};
use anyhow::{Context, Result};
use humansize::format_size;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// Directories we never descend into unless the name itself is an artifact
// target (in that case it is reported as a result and never descended into).
pub const GLOBAL_IGNORE: &[&str] = &[
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

// Absolute path prefixes treated as sensitive by default. They are skipped
// unless explicitly enabled with --allow-system-paths, except when the scan
// root itself lives under one of them (the user asked for that path).
pub const SENSITIVE_SYSTEM_PREFIXES: &[&str] = &[
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

// Files that mark a directory as a project root. The nearest ancestor of an
// artifact containing one of these is the project that owns the artifact.
pub const PROJECT_MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "composer.json",
    "Gemfile",
    "mix.exs",
    "pubspec.yaml",
    "CMakeLists.txt",
    ".git",
];

#[derive(Debug, Clone)]
pub struct Artifact {
    pub path: PathBuf,
    pub size: u64,
    /// Directory name that matched (e.g. "node_modules").
    pub name: String,
    /// Pattern that claimed this directory (e.g. "node_modules", "rust-target").
    pub pattern_name: String,
    pub modified: Option<SystemTime>,
    pub is_safe: bool,
    /// Project this artifact belongs to (nearest ancestor with a marker file).
    pub project: PathBuf,
}

impl Artifact {
    pub fn size_string(&self) -> String {
        format_size(self.size, humansize::BINARY)
    }

    /// Path relative to the scan root, for compact display.
    pub fn relative_to(&self, root: &Path) -> PathBuf {
        self.path
            .strip_prefix(root)
            .unwrap_or(&self.path)
            .to_path_buf()
    }

    pub fn project_relative_to(&self, root: &Path) -> PathBuf {
        self.project
            .strip_prefix(root)
            .unwrap_or(&self.project)
            .to_path_buf()
    }
}

/// A directory skipped by configuration, kept for reporting.
#[derive(Debug, Clone)]
pub struct Blocked {
    pub path: PathBuf,
    pub name: String,
    pub project: PathBuf,
    pub reason: String,
}

/// Artifacts grouped under their owning project, biggest project first.
#[derive(Debug, Clone)]
pub struct ProjectGroup {
    pub root: PathBuf,
    pub artifacts: Vec<Artifact>,
    pub total_size: u64,
}

impl ProjectGroup {
    pub fn size_string(&self) -> String {
        format_size(self.total_size, humansize::BINARY)
    }
}

pub struct ScanOutcome {
    pub artifacts: Vec<Artifact>,
    pub blocked: Vec<Blocked>,
}

impl ScanOutcome {
    /// Group artifacts by project, largest project first; inside a project the
    /// largest artifact comes first.
    pub fn groups(&self) -> Vec<ProjectGroup> {
        let mut groups: Vec<ProjectGroup> = Vec::new();
        for artifact in &self.artifacts {
            match groups.iter_mut().find(|g| g.root == artifact.project) {
                Some(g) => {
                    g.total_size += artifact.size;
                    g.artifacts.push(artifact.clone());
                }
                None => groups.push(ProjectGroup {
                    root: artifact.project.clone(),
                    total_size: artifact.size,
                    artifacts: vec![artifact.clone()],
                }),
            }
        }
        groups.sort_by(|a, b| {
            b.total_size
                .cmp(&a.total_size)
                .then_with(|| a.root.cmp(&b.root))
        });
        for g in &mut groups {
            g.artifacts
                .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
        }
        groups
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
    project_rules: Vec<ProjectRule>,
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
            project_rules: Vec::new(),
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

    pub fn with_project_rules(mut self, rules: Vec<ProjectRule>) -> Self {
        self.project_rules = rules;
        self
    }

    /// Directory this scanner was created for.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Scan the root and return every artifact directory, largest first.
    ///
    /// Traversal is depth-first and stops at the first directory that matches a
    /// pattern: a matched directory is reported once and never descended into,
    /// so results are disjoint (no "folder inside folder").
    pub fn scan(&self) -> Result<ScanOutcome> {
        let mut artifacts = Vec::new();
        let mut blocked = Vec::new();
        let mut cache: HashMap<PathBuf, PathBuf> = HashMap::new();
        // If the user pointed us at (or under) a sensitive prefix, honour it for
        // the whole scan instead of instantly skipping every child.
        let root_sensitive = is_sensitive_system_path(&self.root);

        let mut entries = read_dirs(&self.root);
        // `read_dir` is not sorted; sort for stable output and reproducible tests.
        entries.sort();
        self.walk(
            &entries,
            root_sensitive,
            &mut cache,
            &mut artifacts,
            &mut blocked,
        );

        artifacts.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));

        Ok(ScanOutcome { artifacts, blocked })
    }

    fn walk(
        &self,
        dirs: &[PathBuf],
        root_sensitive: bool,
        cache: &mut HashMap<PathBuf, PathBuf>,
        artifacts: &mut Vec<Artifact>,
        blocked: &mut Vec<Blocked>,
    ) {
        for path in dirs {
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            // 1. .klignore is an explicit protection list: it wins over patterns.
            if self.ignore_rules.protects() && self.ignore_rules.is_ignored(path) {
                continue;
            }

            // 2. Sensitive system paths (never skips the scan root itself).
            if !root_sensitive && !self.allow_system_paths && is_sensitive_system_path(path) {
                continue;
            }

            // 3. A directory matching a pattern is an artifact: report it and
            //    never descend into it.
            if let Some(pattern) = self.find_pattern(&name) {
                let project = self.resolve_project(path, cache);
                match self.project_rule_for(&project, &name, &pattern.name) {
                    Some(rule) => {
                        blocked.push(Blocked {
                            path: path.clone(),
                            name: name.clone(),
                            project: project.clone(),
                            reason: rule,
                        });
                    }
                    None => {
                        let size = calculate_dir_size(path);
                        if self.size_allowed(size) {
                            artifacts.push(Artifact {
                                path: path.clone(),
                                size,
                                name: name.clone(),
                                pattern_name: pattern.name.clone(),
                                modified: fs::metadata(path).and_then(|m| m.modified()).ok(),
                                is_safe: pattern.safe_to_delete,
                                project,
                            });
                        }
                    }
                }
                continue;
            }

            // 4. Noise directories are pruned (contents never scanned).
            if GLOBAL_IGNORE.contains(&name.as_str()) {
                continue;
            }

            // 5. .gitignore is only a traversal hint, never a protection list:
            //    a gitignored artifact is still reported, but we stop there.
            if self.ignore_rules.is_ignored(path) {
                continue;
            }

            let mut children = read_dirs(path);
            children.sort();
            self.walk(&children, root_sensitive, cache, artifacts, blocked);
        }
    }

    /// First pattern whose pattern list matches the directory name.
    fn find_pattern(&self, name: &str) -> Option<&ArtifactPattern> {
        self.patterns.iter().find(|pattern| {
            if let Some(ref filter) = self.filter {
                let matches_filter = pattern.name.contains(filter.as_str())
                    || pattern.patterns.iter().any(|p| p.contains(filter.as_str()));
                if !matches_filter {
                    return false;
                }
            }
            pattern.patterns.iter().any(|p| name_matches(name, p))
        })
    }

    fn size_allowed(&self, size: u64) -> bool {
        if let Some(min) = self.min_size {
            if size < min {
                return false;
            }
        }
        if let Some(max) = self.max_size {
            if size > max {
                return false;
            }
        }
        true
    }

    /// Nearest ancestor of the artifact that looks like a project root; falls
    /// back to the artifact's own parent when nothing matches.
    fn resolve_project(&self, artifact: &Path, cache: &mut HashMap<PathBuf, PathBuf>) -> PathBuf {
        let start = artifact
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.root.clone());

        if let Some(hit) = cache.get(&start) {
            return hit.clone();
        }

        let mut current = start.clone();
        let found = loop {
            if has_project_marker(&current) {
                break current.clone();
            }
            if current == self.root {
                break start.clone();
            }
            match current.parent() {
                Some(parent) if parent.starts_with(&self.root) => current = parent.to_path_buf(),
                _ => break start.clone(),
            }
        };

        cache.insert(start, found.clone());
        found
    }

    /// `Some(reason)` when a project rule forbids deleting this artifact.
    fn project_rule_for(&self, project: &Path, name: &str, pattern_name: &str) -> Option<String> {
        // Most specific rule wins (longest matching path).
        let (rule, _) = self
            .project_rules
            .iter()
            .filter_map(|r| r.matched_len(project).map(|len| (r, len)))
            .max_by_key(|(_, len)| *len)?;

        if rule.enabled == Some(false) {
            return Some(format!("projeto desativado pela regra `{}`", rule.path));
        }

        let hits = |list: &[String]| -> bool {
            list.iter()
                .any(|item| name_matches(name, item) || name_matches(pattern_name, item))
        };

        if rule.deny.as_ref().is_some_and(|deny| hits(deny)) {
            return Some(format!("negado pela regra `{}`", rule.path));
        }

        if let Some(allow) = &rule.allow {
            if !allow.is_empty() && !hits(allow) {
                return Some(format!("fora da allow-list da regra `{}`", rule.path));
            }
        }

        None
    }
}

fn read_dirs(path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return dirs,
    };
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(entry.path());
        }
    }
    dirs
}

fn has_project_marker(dir: &Path) -> bool {
    PROJECT_MARKERS
        .iter()
        .any(|marker| dir.join(marker).exists())
}

pub fn is_sensitive_system_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    SENSITIVE_SYSTEM_PREFIXES
        .iter()
        .any(|prefix| path_str == *prefix || path_str.starts_with(&format!("{}/", prefix)))
}

/// Calculate total size of a directory recursively
pub fn calculate_dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(entry.path()),
                Ok(ft) if ft.is_file() => {
                    if let Ok(md) = entry.metadata() {
                        total += md.len();
                    }
                }
                _ => {}
            }
        }
    }
    total
}

/// Parse size string like "100MB", "1GB" to bytes
pub fn parse_size(size_str: &str) -> Result<u64> {
    let upper = size_str.to_uppercase();
    let size_str = upper.trim();

    let (num_str, unit) = if let Some(rest) = size_str.strip_suffix("GB") {
        (rest, 1024u64 * 1024 * 1024)
    } else if let Some(rest) = size_str.strip_suffix("MB") {
        (rest, 1024u64 * 1024)
    } else if let Some(rest) = size_str.strip_suffix("KB") {
        (rest, 1024u64)
    } else if let Some(rest) = size_str.strip_suffix('B') {
        (rest, 1u64)
    } else {
        // Try parsing as plain number (bytes)
        (size_str, 1u64)
    };

    let num: u64 = num_str
        .trim()
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
            project: PathBuf::from("/tmp"),
        };
        assert!(artifact.size_string().contains("MiB"));
    }
}
