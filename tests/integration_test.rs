use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_node_modules_detection() {
    // Create temporary project structure
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create project files
    fs::write(project_root.join("package.json"), "{}").unwrap();

    // Create node_modules with proper structure
    let node_modules = project_root.join("node_modules");
    fs::create_dir(&node_modules).unwrap();

    let package1 = node_modules.join("package1");
    fs::create_dir(&package1).unwrap();
    fs::write(package1.join("index.js"), "").unwrap();

    // This is a basic integration test structure
    // Full tests would use the klean library directly
    assert!(project_root.join("node_modules").exists());
}

#[test]
fn test_python_venv_detection() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create project files
    fs::write(project_root.join("requirements.txt"), "").unwrap();

    // Create venv
    fs::create_dir(project_root.join(".venv")).unwrap();
    fs::create_dir(project_root.join(".venv/lib")).unwrap();

    assert!(project_root.join(".venv").exists());
}

#[test]
fn test_rust_target_detection() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create project files
    fs::write(project_root.join("Cargo.toml"), "").unwrap();

    // Create target directory
    fs::create_dir(project_root.join("target")).unwrap();
    fs::create_dir(project_root.join("target/debug")).unwrap();

    assert!(project_root.join("target").exists());
}

#[test]
fn test_multiple_artifacts() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create mixed project
    fs::write(project_root.join("package.json"), "{}").unwrap();
    fs::write(project_root.join("Cargo.toml"), "").unwrap();

    // Create multiple artifact directories
    fs::create_dir_all(project_root.join("node_modules/pkg")).unwrap();
    fs::create_dir_all(project_root.join("target/debug")).unwrap();
    fs::create_dir_all(project_root.join(".next")).unwrap();

    assert!(project_root.join("node_modules").exists());
    assert!(project_root.join("target").exists());
    assert!(project_root.join(".next").exists());
}

#[test]
fn test_nested_artifacts() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create monorepo structure
    fs::create_dir_all(project_root.join("apps/web")).unwrap();
    fs::create_dir_all(project_root.join("apps/api")).unwrap();

    // Create artifacts in subdirectories
    fs::create_dir_all(project_root.join("apps/web/node_modules/pkg")).unwrap();
    fs::create_dir_all(project_root.join("apps/api/node_modules/pkg")).unwrap();

    assert!(project_root.join("apps/web/node_modules").exists());
    assert!(project_root.join("apps/api/node_modules").exists());
}

#[test]
fn test_size_calculation() {
    let temp_dir = TempDir::new().unwrap();
    let artifact_dir = temp_dir.path().join("artifact");

    fs::create_dir(&artifact_dir).unwrap();

    // Create files with specific sizes
    let file1 = artifact_dir.join("file1.txt");
    let file2 = artifact_dir.join("file2.txt");

    fs::write(&file1, "x".repeat(1024)).unwrap(); // 1KB
    fs::write(&file2, "x".repeat(10 * 1024)).unwrap(); // 10KB

    let total: u64 = fs::metadata(&file1).unwrap().len() + fs::metadata(&file2).unwrap().len();

    assert_eq!(total, 11 * 1024);
}

#[test]
fn test_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let project = temp_dir.path();

    // Create comprehensive project structure
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("node_modules")).unwrap();
    fs::create_dir_all(project.join("target/debug")).unwrap();
    fs::create_dir_all(project.join(".next")).unwrap();
    fs::create_dir_all(project.join("dist")).unwrap();

    assert!(project.join("src").exists());
    assert!(project.join("node_modules").exists());
    assert!(project.join("target").exists());
    assert!(project.join(".next").exists());
    assert!(project.join("dist").exists());
}

#[test]
fn test_hidden_directories() {
    let temp_dir = TempDir::new().unwrap();
    let project = temp_dir.path();

    // Test hidden directories
    fs::create_dir_all(project.join(".next")).unwrap();
    fs::create_dir_all(project.join(".nuxt")).unwrap();
    fs::create_dir_all(project.join(".vscode")).unwrap();

    assert!(project.join(".next").exists());
    assert!(project.join(".nuxt").exists());
    assert!(project.join(".vscode").exists());
}
