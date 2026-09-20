//! Behavior tests for the scanner: no nested results, project grouping and
//! per-project deletion rules.

use klean::config::ProjectRule;
use klean::ignore::IgnoreRules;
use klean::patterns::get_default_patterns;
use klean::scanner::{ArtifactScanner, ScanOutcome};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn write_file(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
}

fn scan(root: &Path) -> ScanOutcome {
    scan_with(root, Vec::new())
}

fn scan_with(root: &Path, rules: Vec<ProjectRule>) -> ScanOutcome {
    let ignore = IgnoreRules::from_path(root, true).unwrap();
    ArtifactScanner::new(root.to_path_buf(), ignore, get_default_patterns())
        .with_project_rules(rules)
        .scan()
        .unwrap()
}

/// Monorepo with artifacts nested inside other artifacts.
fn fixture() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    write_file(&root.join("apps/web/package.json"), 10);
    write_file(&root.join("apps/web/node_modules/left-pad/f1.js"), 2000);
    write_file(
        &root.join("apps/web/node_modules/left-pad/node_modules/trim/f2.js"),
        2000,
    );

    write_file(&root.join("apps/api/Cargo.toml"), 10);
    write_file(
        &root.join("apps/api/target/debug/build/serde/f3.rlib"),
        3000,
    );

    write_file(&root.join("py/requirements.txt"), 10);
    write_file(&root.join("py/.venv/lib/pkg/__pycache__/m.pyc"), 2000);
    write_file(&root.join("py/mypkg.egg-info/PKG-INFO"), 1000);

    (tmp, root)
}

fn names(outcome: &ScanOutcome) -> Vec<String> {
    let mut v: Vec<String> = outcome
        .artifacts
        .iter()
        .map(|a| a.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

#[test]
fn finds_every_artifact_once_and_never_nests() {
    let (_tmp, root) = fixture();
    let outcome = scan(&root);

    // Nested artifacts (node_modules/left-pad/node_modules, target/debug/build,
    // .venv/**/__pycache__) must not show up as separate results.
    assert_eq!(
        names(&outcome),
        vec![".venv", "mypkg.egg-info", "node_modules", "target"],
        "unexpected artifact set: {:?}",
        names(&outcome)
    );

    // Disjoint results: deleting one can never invalidate another.
    for (i, a) in outcome.artifacts.iter().enumerate() {
        for b in outcome.artifacts.iter().skip(i + 1) {
            assert!(
                !b.path.starts_with(&a.path) && !a.path.starts_with(&b.path),
                "nested artifacts: {} inside {}",
                b.path.display(),
                a.path.display()
            );
        }
    }
}

#[test]
fn groups_by_project_largest_first() {
    let (_tmp, root) = fixture();
    let outcome = scan(&root);
    let groups = outcome.groups();

    let got: Vec<(String, u64)> = groups
        .iter()
        .map(|g| {
            (
                g.root.file_name().unwrap().to_string_lossy().to_string(),
                g.total_size,
            )
        })
        .collect();

    assert_eq!(
        got,
        vec![
            ("web".to_string(), 4000),
            ("api".to_string(), 3000),
            ("py".to_string(), 3000),
        ]
    );

    // Inside a project the largest artifact comes first.
    let py = groups.last().unwrap();
    assert_eq!(py.artifacts[0].name, ".venv");
}

#[test]
fn project_rules_gate_deletion_per_project() {
    let (_tmp, root) = fixture();

    let deny_venv = ProjectRule {
        path: root.join("py").display().to_string(),
        deny: Some(vec![".venv".to_string()]),
        ..Default::default()
    };
    let outcome = scan_with(&root, vec![deny_venv.clone()]);
    assert!(!names(&outcome).contains(&".venv".to_string()));
    assert!(names(&outcome).contains(&"mypkg.egg-info".to_string()));
    assert_eq!(outcome.blocked.len(), 1);

    // allow-list: only listed artifacts survive
    let allow_venv_only = ProjectRule {
        path: root.join("py").display().to_string(),
        allow: Some(vec!["python-venv".to_string()]),
        ..Default::default()
    };
    let outcome = scan_with(&root, vec![allow_venv_only]);
    assert_eq!(
        names(&outcome)
            .iter()
            .filter(|n| n.as_str() == ".venv")
            .count(),
        1
    );
    assert!(!names(&outcome).contains(&"mypkg.egg-info".to_string()));

    // disabled project: nothing from it is reported
    let disabled = ProjectRule {
        path: root.join("py").display().to_string(),
        enabled: Some(false),
        ..Default::default()
    };
    let outcome = scan_with(&root, vec![disabled]);
    assert!(!outcome.artifacts.iter().any(|a| a.project.ends_with("py")));
}

#[test]
fn klignore_protects_artifacts_and_gitignore_does_not() {
    let (_tmp, root) = fixture();
    fs::write(root.join(".klignore"), "/apps/web/node_modules\n").unwrap();

    let outcome = scan(&root);
    assert!(
        !names(&outcome).contains(&"node_modules".to_string()),
        ".klignore must protect node_modules, got {:?}",
        names(&outcome)
    );

    // .gitignore is common in real repos (node_modules is always ignored) and
    // must not hide artifacts.
    fs::remove_file(root.join(".klignore")).unwrap();
    fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
    let outcome = scan(&root);
    assert!(names(&outcome).contains(&"node_modules".to_string()));
}

/// The catalogue is only worth anything if every entry is actually reachable:
/// adds a directory per ecosystem and demands all of them, exactly once.
#[test]
fn detects_each_ecosystem_directory_once() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    let cases: &[(&str, &str)] = &[
        ("js", "node_modules"),
        ("js", ".next"),
        ("js", ".svelte-kit"),
        ("js", ".turbo"),
        ("js", "bower_components"),
        ("js", ".pnpm-store"),
        ("py", "__pycache__"),
        ("py", ".mypy_cache"),
        ("py", ".nox"),
        ("py", ".ipynb_checkpoints"),
        ("rs", "target"),
        ("jvm", ".gradle"),
        ("jvm", ".kotlin"),
        ("jvm", "out"),
        ("scala", ".scala-build"),
        ("clj", ".cpcache"),
        ("cs", "obj"),
        ("cs", "BenchmarkDotNet.Artifacts"),
        ("cpp", "cmake-build-debug"),
        ("cpp", "_deps"),
        ("apple", "Pods"),
        ("apple", "DerivedData"),
        ("dart", ".dart_tool"),
        ("unity", "Library"),
        ("godot", ".godot"),
        ("ex", "_build"),
        ("ocaml", "_opam"),
        ("hs", ".stack-work"),
        ("zig", "zig-cache"),
        ("nim", "nimcache"),
        ("rb", ".yardoc"),
        ("php", ".phpunit.cache"),
        ("tf", ".terraform"),
        ("ml", "mlruns"),
        ("gen", ".idea"),
        ("gen", ".cache"),
        ("gen", "coverage"),
    ];

    for (project, dir) in cases {
        write_file(&root.join(project).join(dir).join("payload.bin"), 1024);
    }

    let outcome = scan(&root);
    let found: Vec<String> = outcome.artifacts.iter().map(|a| a.name.clone()).collect();

    for (project, dir) in cases {
        assert!(
            found.contains(&dir.to_string()),
            "{dir} in {project} was not detected (got {found:?})"
        );
    }

    assert_eq!(
        outcome.artifacts.len(),
        cases.len(),
        "one artifact per directory and nothing nested, got {found:?}"
    );
}
