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

/// Every pattern is a *directory name* (exact or `*`/`?` glob): klean reports a
/// matched directory once and never descends into it.
///
/// `safe_to_delete: false` marks names that are usually regenerable but could
/// hold work someone cares about (`out`, `bin`, `Library`, `mlruns`): they are
/// still listed and can be cleaned, with a warning.
pub static DEFAULT_PATTERNS: Lazy<Vec<ArtifactPattern>> = Lazy::new(|| {
    let pattern = |name: &str,
                   patterns: &[&str],
                   languages: &[&str],
                   description: &str,
                   safe_to_delete: bool| ArtifactPattern {
        name: name.to_string(),
        patterns: patterns.iter().map(|p| p.to_string()).collect(),
        languages: languages.iter().map(|l| l.to_string()).collect(),
        description: description.to_string(),
        safe_to_delete,
    };

    vec![
        // ---------- JavaScript / TypeScript ----------
        pattern(
            "node_modules",
            &["node_modules"],
            &["JavaScript", "TypeScript"],
            "Node.js dependencies",
            true,
        ),
        pattern("npm-cache", &[".npm"], &["JavaScript"], "npm cache", true),
        pattern(
            "pnpm-store",
            &[".pnpm-store"],
            &["JavaScript", "TypeScript"],
            "pnpm content-addressable store",
            true,
        ),
        pattern(
            "bower",
            &["bower_components"],
            &["JavaScript"],
            "Bower dependencies",
            true,
        ),
        pattern(
            "nextjs-build",
            &[".next"],
            &["JavaScript", "TypeScript"],
            "Next.js build output",
            true,
        ),
        pattern(
            "nuxt-build",
            &[".nuxt", ".output"],
            &["JavaScript", "Vue"],
            "Nuxt build output",
            true,
        ),
        pattern(
            "svelte-kit",
            &[".svelte-kit"],
            &["JavaScript", "TypeScript", "Svelte"],
            "SvelteKit build output",
            true,
        ),
        pattern(
            "astro-build",
            &[".astro"],
            &["JavaScript", "TypeScript"],
            "Astro build cache",
            true,
        ),
        pattern(
            "angular-cache",
            &[".angular"],
            &["TypeScript"],
            "Angular build cache",
            true,
        ),
        pattern(
            "docusaurus",
            &[".docusaurus"],
            &["JavaScript", "TypeScript"],
            "Docusaurus build cache",
            true,
        ),
        pattern(
            "vite-cache",
            &[".vite"],
            &["JavaScript", "TypeScript"],
            "Vite dependency cache",
            true,
        ),
        pattern(
            "bundler-caches",
            &[".turbo", ".parcel-cache", ".webpack", ".rollup.cache"],
            &["JavaScript", "TypeScript"],
            "Bundler caches (Turborepo, Parcel, webpack, Rollup)",
            true,
        ),
        pattern(
            "storybook",
            &["storybook-static"],
            &["JavaScript", "TypeScript"],
            "Storybook static build",
            true,
        ),
        pattern(
            "expo",
            &[".expo"],
            &["JavaScript", "TypeScript"],
            "Expo build cache",
            true,
        ),
        pattern(
            "hosting-cli",
            &[".vercel", ".netlify", ".serverless", ".aws-sam"],
            &["JavaScript", "TypeScript", "Generic"],
            "Hosting/serverless CLI build output",
            true,
        ),
        pattern(
            "sass-cache",
            &[".sass-cache"],
            &["CSS", "SCSS"],
            "Sass compilation cache",
            true,
        ),
        // ---------- Python ----------
        pattern(
            "pycache",
            &["__pycache__"],
            &["Python"],
            "Python bytecode cache",
            true,
        ),
        pattern(
            "python-venv",
            &[".venv", "venv", ".tox", ".nox", "env"],
            &["Python"],
            "Python virtual environments",
            true,
        ),
        pattern(
            "python-dist",
            &["*.egg-info", "dist", "build", "develop-eggs"],
            &["Python"],
            "Python packaging output",
            true,
        ),
        pattern(
            "python-tool-caches",
            &[
                ".mypy_cache",
                ".ruff_cache",
                ".pytest_cache",
                ".pytype",
                ".pyre",
                ".hypothesis",
                "htmlcov",
            ],
            &["Python"],
            "Python tooling caches (mypy, ruff, pytest, hypothesis)",
            true,
        ),
        pattern(
            "python-misc",
            &[
                ".ipynb_checkpoints",
                "__pypackages__",
                ".eggs",
                "pip-wheel-metadata",
            ],
            &["Python", "Jupyter"],
            "Notebook checkpoints and legacy packaging dirs",
            true,
        ),
        // ---------- Rust / Go ----------
        pattern(
            "rust-target",
            &["target"],
            &["Rust"],
            "Rust build output",
            true,
        ),
        // ---------- JVM ----------
        pattern(
            "gradle-build",
            &["build", ".gradle", ".kotlin"],
            &["Java", "Kotlin", "Groovy"],
            "Gradle build output and caches",
            true,
        ),
        pattern(
            "maven-target",
            &["target"],
            &["Java"],
            "Maven build output",
            true,
        ),
        pattern(
            "jvm-ide-output",
            &["out"],
            &["Java", "Kotlin", "Scala"],
            "IDE compiler output (IntelliJ)",
            false,
        ),
        pattern(
            "scala-build",
            &[".scala-build", ".bloop", ".metals", ".bsp"],
            &["Scala"],
            "Scala CLI/Bloop/Metals build output",
            true,
        ),
        pattern(
            "clojure-build",
            &[".cpcache", ".clj-kondo"],
            &["Clojure"],
            "Clojure classpath and linter caches",
            true,
        ),
        // ---------- .NET ----------
        pattern(
            "dotnet-build",
            &["obj", ".vs", "TestResults", "BenchmarkDotNet.Artifacts"],
            &["C#", "F#", "VB.NET"],
            ".NET intermediate output and test results",
            true,
        ),
        // ---------- C / C++ ----------
        pattern(
            "cpp-build",
            &[
                "CMakeFiles",
                "cmake-build-debug",
                "cmake-build-release",
                "_deps",
                "autom4te.cache",
                ".deps",
                ".libs",
                "builddir",
            ],
            &["C", "C++", "Objective-C"],
            "CMake/Autotools/Meson build output",
            true,
        ),
        // ---------- Apple ----------
        pattern(
            "apple-build",
            &[
                "DerivedData",
                "Pods",
                "Carthage",
                ".build",
                "Build",
                "xcuserdata",
            ],
            &["Swift", "Objective-C"],
            "Xcode/SwiftPM/CocoaPods build output",
            true,
        ),
        // ---------- Android / Dart ----------
        pattern(
            "android-build",
            &[".cxx", ".externalNativeBuild", "captures"],
            &["Kotlin", "Java"],
            "Android native build output",
            true,
        ),
        pattern(
            "dart-flutter",
            &[".dart_tool", ".pub-cache"],
            &["Dart", "Flutter"],
            "Dart/Flutter build cache",
            true,
        ),
        // ---------- Game engines ----------
        pattern(
            "unity",
            &["Library", "Temp", "Logs", "Builds"],
            &["C#", "Unity"],
            "Unity project cache and builds",
            false,
        ),
        pattern(
            "godot",
            &[".godot", ".import", ".mono"],
            &["GDScript", "Godot"],
            "Godot import/compile cache",
            true,
        ),
        // ---------- BEAM / functional ----------
        pattern(
            "elixir-build",
            &["_build", "deps", ".elixir_ls", "cover"],
            &["Elixir", "Erlang"],
            "Elixir/Erlang build output and dependencies",
            true,
        ),
        pattern(
            "ocaml-build",
            &["_opam"],
            &["OCaml"],
            "OCaml local switch packages",
            true,
        ),
        pattern(
            "haskell-build",
            &[".stack-work", "dist-newstyle"],
            &["Haskell"],
            "Stack/Cabal build output",
            true,
        ),
        pattern(
            "zig-build",
            &["zig-cache", "zig-out", ".zig-cache"],
            &["Zig"],
            "Zig build cache and output",
            true,
        ),
        pattern(
            "nim-build",
            &["nimcache"],
            &["Nim"],
            "Nim compilation cache",
            true,
        ),
        pattern(
            "purescript-output",
            &["output"],
            &["PureScript"],
            "PureScript compiler output",
            false,
        ),
        // ---------- Ruby / PHP ----------
        pattern(
            "bundler-gems",
            &[".bundle", ".yardoc"],
            &["Ruby"],
            "Bundler/YARD caches",
            true,
        ),
        pattern(
            "composer-vendor",
            &["vendor"],
            &["PHP"],
            "Composer dependencies",
            true,
        ),
        pattern(
            "php-tool-caches",
            &[".phpunit.cache", ".php-cs-fixer.cache", ".phpstan-cache"],
            &["PHP"],
            "PHP tooling caches (PHPUnit, CS-Fixer, PHPStan)",
            true,
        ),
        // ---------- Infra ----------
        pattern(
            "terraform",
            &[".terraform", ".terragrunt-cache"],
            &["Terraform", "HCL"],
            "Terraform provider and plugin cache",
            true,
        ),
        // ---------- Experiments / data ----------
        pattern(
            "experiment-tracking",
            &["mlruns", "wandb", "lightning_logs", ".lightning"],
            &["Python", "ML"],
            "ML experiment logs and checkpoints (keep if unreproducible!)",
            false,
        ),
        // ---------- Generic ----------
        pattern(
            "dist",
            &["dist"],
            &["JavaScript", "TypeScript", "Generic"],
            "Distribution/build output",
            true,
        ),
        pattern(
            "coverage",
            &["coverage", ".nyc_output"],
            &["Generic"],
            "Test coverage reports",
            true,
        ),
        pattern(
            "cache",
            &[".cache"],
            &["Generic"],
            "Generic hidden cache directory",
            true,
        ),
        pattern(
            "ide-project-metadata",
            &[".idea"],
            &["Generic"],
            "JetBrains project index and local settings",
            false,
        ),
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

    #[test]
    fn patterns_are_unique_and_described() {
        let patterns = get_default_patterns();

        let mut names: Vec<&str> = patterns.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped, "duplicate pattern names");

        for pattern in &patterns {
            assert!(
                !pattern.patterns.is_empty(),
                "{} matches nothing",
                pattern.name
            );
            assert!(
                !pattern.description.is_empty(),
                "{} has no description",
                pattern.name
            );
            assert!(
                !pattern.languages.is_empty(),
                "{} has no language",
                pattern.name
            );
            for entry in &pattern.patterns {
                assert!(
                    !entry.is_empty() && !entry.contains('/'),
                    "{} has an unusable pattern `{}`: artifacts are matched by \
                     directory name, a path never matches",
                    pattern.name,
                    entry
                );
            }
        }
    }

    #[test]
    fn every_ecosystem_has_a_pattern() {
        // The list is a catalogue: a language without a pattern is a gap.
        let names = get_pattern_names();
        for wanted in [
            "node_modules",
            "python-venv",
            "rust-target",
            "gradle-build",
            "dotnet-build",
            "cpp-build",
            "apple-build",
            "dart-flutter",
            "unity",
            "elixir-build",
            "haskell-build",
            "composer-vendor",
            "bundler-gems",
            "terraform",
            "zig-build",
        ] {
            assert!(names.contains(&wanted.to_string()), "{wanted} is missing");
        }
    }
}
