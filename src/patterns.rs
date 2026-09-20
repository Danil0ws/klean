use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// Represents a single artifact pattern with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPattern {
    pub name: String,
    pub patterns: Vec<String>,
    pub languages: Vec<String>,
    pub description: String,
    pub safe_to_delete: bool,
}

pub static DEFAULT_PATTERNS: Lazy<Vec<ArtifactPattern>> = Lazy::new(|| {
    vec![
        // Node.js
        ArtifactPattern {
            name: "node_modules".to_string(),
            patterns: vec!["node_modules".to_string()],
            languages: vec!["JavaScript".to_string(), "TypeScript".to_string()],
            description: "Node.js dependencies directory".to_string(),
            safe_to_delete: true,
        },
        ArtifactPattern {
            name: "npm-cache".to_string(),
            patterns: vec![".npm".to_string()],
            languages: vec!["JavaScript".to_string()],
            description: "NPM cache directory".to_string(),
            safe_to_delete: true,
        },
        // Python
        ArtifactPattern {
            name: "pycache".to_string(),
            patterns: vec!["__pycache__".to_string()],
            languages: vec!["Python".to_string()],
            description: "Python bytecode cache".to_string(),
            safe_to_delete: true,
        },
        ArtifactPattern {
            name: "python-venv".to_string(),
            patterns: vec![".venv".to_string(), "venv".to_string(), ".tox".to_string()],
            languages: vec!["Python".to_string()],
            description: "Python virtual environments".to_string(),
            safe_to_delete: true,
        },
        ArtifactPattern {
            name: "python-dist".to_string(),
            patterns: vec![
                "*.egg-info".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            languages: vec!["Python".to_string()],
            description: "Python distribution and build artifacts".to_string(),
            safe_to_delete: true,
        },
        // Rust
        ArtifactPattern {
            name: "rust-target".to_string(),
            patterns: vec!["target".to_string()],
            languages: vec!["Rust".to_string()],
            description: "Rust build artifacts directory".to_string(),
            safe_to_delete: true,
        },
        // Java/Kotlin/Gradle
        ArtifactPattern {
            name: "gradle-build".to_string(),
            patterns: vec!["build".to_string(), ".gradle".to_string()],
            languages: vec!["Java".to_string(), "Kotlin".to_string()],
            description: "Gradle build artifacts".to_string(),
            safe_to_delete: true,
        },
        ArtifactPattern {
            name: "maven-target".to_string(),
            patterns: vec!["target".to_string()],
            languages: vec!["Java".to_string()],
            description: "Maven target directory".to_string(),
            safe_to_delete: true,
        },
        // C#
        ArtifactPattern {
            name: "dotnet-build".to_string(),
            patterns: vec![
                "obj".to_string(),
                ".vs".to_string(),
                "TestResults".to_string(),
            ],
            languages: vec!["C#".to_string(), "CSharp".to_string()],
            description: ".NET build artifacts".to_string(),
            safe_to_delete: true,
        },
        // C++
        ArtifactPattern {
            name: "cpp-build".to_string(),
            patterns: vec!["CMakeFiles".to_string(), "cmake_install.cmake".to_string()],
            languages: vec!["C++".to_string()],
            description: "CMake build artifacts".to_string(),
            safe_to_delete: true,
        },
        // Next.js/Nuxt
        ArtifactPattern {
            name: "nextjs-build".to_string(),
            patterns: vec![".next".to_string()],
            languages: vec!["JavaScript".to_string(), "TypeScript".to_string()],
            description: "Next.js build cache".to_string(),
            safe_to_delete: true,
        },
        ArtifactPattern {
            name: "nuxt-build".to_string(),
            patterns: vec![".nuxt".to_string()],
            languages: vec!["JavaScript".to_string(), "Vue".to_string()],
            description: "Nuxt build artifacts".to_string(),
            safe_to_delete: true,
        },
        // PHP
        ArtifactPattern {
            name: "composer-vendor".to_string(),
            patterns: vec!["vendor".to_string()],
            languages: vec!["PHP".to_string()],
            description: "Composer vendor directory".to_string(),
            safe_to_delete: true,
        },
        // Ruby
        ArtifactPattern {
            name: "bundler-gems".to_string(),
            patterns: vec![".bundle".to_string()],
            languages: vec!["Ruby".to_string()],
            description: "Bundler gem cache".to_string(),
            safe_to_delete: true,
        },
        // Elixir
        ArtifactPattern {
            name: "elixir-build".to_string(),
            patterns: vec!["_build".to_string(), "deps".to_string()],
            languages: vec!["Elixir".to_string()],
            description: "Elixir build and dependencies".to_string(),
            safe_to_delete: true,
        },
        // Haskell
        ArtifactPattern {
            name: "haskell-build".to_string(),
            patterns: vec![".stack-work".to_string(), "dist-newstyle".to_string()],
            languages: vec!["Haskell".to_string()],
            description: "Haskell build artifacts".to_string(),
            safe_to_delete: true,
        },
        // Mobile
        ArtifactPattern {
            name: "android-build".to_string(),
            patterns: vec![".android".to_string(), "build".to_string()],
            languages: vec!["Kotlin".to_string(), "Java".to_string()],
            description: "Android build artifacts".to_string(),
            safe_to_delete: true,
        },
        ArtifactPattern {
            name: "ios-build".to_string(),
            patterns: vec![".ios".to_string(), "DerivedData".to_string()],
            languages: vec!["Swift".to_string(), "Objective-C".to_string()],
            description: "iOS build artifacts".to_string(),
            safe_to_delete: true,
        },
        // Build artifacts
        ArtifactPattern {
            name: "dist".to_string(),
            patterns: vec!["dist".to_string()],
            languages: vec!["JavaScript".to_string(), "TypeScript".to_string()],
            description: "Distribution build directory".to_string(),
            safe_to_delete: true,
        },
        // IDE and editor
        // NOTE: `.vscode/extensions` used to live here but could never match a
        // directory name, and .vscode holds user settings: not an artifact.
        // General cache directories
        ArtifactPattern {
            name: "cache".to_string(),
            patterns: vec![".cache".to_string()],
            languages: vec!["Generic".to_string()],
            description: "Generic hidden cache directory".to_string(),
            safe_to_delete: true,
        },
    ]
});

/// Match a directory name against a pattern. Supports exact names and simple
/// globs (`*`, `?`), so patterns like `*.egg-info` work.
pub fn name_matches(name: &str, pattern: &str) -> bool {
    if !pattern.contains('*') && !pattern.contains('?') {
        return name == pattern;
    }

    let name: Vec<char> = name.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    // (name index, pattern index) backtracking for the last `*`.
    let (mut n, mut p) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);

    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            n += 1;
            p += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = n;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            n = mark;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }

    p == pattern.len()
}

/// Get all default patterns
pub fn get_default_patterns() -> Vec<ArtifactPattern> {
    DEFAULT_PATTERNS.clone()
}

/// Get patterns by language
#[allow(dead_code)]
pub fn get_patterns_by_language(language: &str) -> Vec<ArtifactPattern> {
    DEFAULT_PATTERNS
        .iter()
        .filter(|p| {
            p.languages.contains(&language.to_string()) || p.languages.contains(&"".to_string())
        })
        .cloned()
        .collect()
}

/// Get pattern names
#[allow(dead_code)]
pub fn get_pattern_names() -> Vec<String> {
    DEFAULT_PATTERNS.iter().map(|p| p.name.clone()).collect()
}

/// Get patterns by name
#[allow(dead_code)]
pub fn get_patterns_by_name(names: &[String]) -> Vec<ArtifactPattern> {
    DEFAULT_PATTERNS
        .iter()
        .filter(|p| names.contains(&p.name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_default_patterns() {
        let patterns = get_default_patterns();
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_get_patterns_by_language() {
        let patterns = get_patterns_by_language("JavaScript");
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.name == "node_modules"));
    }

    #[test]
    fn test_get_pattern_names() {
        let names = get_pattern_names();
        assert!(names.contains(&"node_modules".to_string()));
    }
}
