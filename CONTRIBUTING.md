# Contributing to klean

Thank you for your interest in contributing to klean! We appreciate your help in making this tool better and safer for everyone.

## Code of Conduct

Please be respectful and constructive in all interactions with the community.

## Getting Started

### Prerequisites

- Rust 1.70+ (stable)
- Git

### Setting Up Development Environment

1. Fork and clone the repository

```bash
git clone https://github.com/YOUR_USERNAME/klean.git
cd klean
```

2. Create a feature branch

```bash
git checkout -b feat/your-feature-name
# or
git checkout -b fix/your-bug-fix
```

3. Install development dependencies

```bash
cargo build
cargo test
```

## Development Workflow

### Making Changes

1. Ensure your changes follow Rust conventions:

```bash
cargo fmt
cargo clippy
```

2. Write tests for new functionality

```bash
# Run all tests
cargo test

# Run specific test
cargo test artifact_patterns

# Run with output
cargo test -- --nocapture
```

3. Update documentation if needed

### Commit Messages

Follow conventional commit format:

```
feat: Add support for new language
fix: Correct size calculation bug
docs: Update README with examples
test: Add integration tests
refactor: Reorganize scanner module
chore: Update dependencies
```

### Pull Request Process

1. Ensure all tests pass: `cargo test`
2. Ensure code is formatted: `cargo fmt`
3. Ensure no clippy warnings: `cargo clippy`
4. Update README.md if adding features
5. Create a descriptive PR with:
   - Clear title
   - Description of changes
   - Related issues (if any)
   - Testing steps

## Areas for Contribution

### 🆕 New Features

- Additional language/framework support
- Alternative UI modes
- New filtering options
- Configuration enhancements

### 🐛 Bug Fixes

- Safety improvements
- Performance optimizations
- Platform-specific issues
- Documentation corrections

### 📚 Documentation

- Tutorial guides
- Configuration examples
- Video walkthroughs
- Blog posts

### ✅ Testing

- Integration tests
- Platform-specific tests (Windows, macOS, Linux)
- Performance benchmarks

## Adding New Artifact Patterns

1. Edit `src/patterns.rs`
2. Add a new `ArtifactPattern` to `DEFAULT_PATTERNS`

```rust
ArtifactPattern {
    name: "my_artifact".to_string(),
    patterns: vec!["my_dir".to_string(), ".my_cache".to_string()],
    languages: vec!["MyLanguage".to_string()],
    description: "My custom artifact description".to_string(),
    safe_to_delete: true,
}
```

3. Add tests:

```rust
#[test]
fn test_my_artifact_pattern() {
    let patterns = get_patterns_by_language("MyLanguage");
    assert!(patterns.iter().any(|p| p.name == "my_artifact"));
}
```

4. Update README.md with the new pattern

## Code Style

### Naming Conventions

- Functions: `snake_case`
- Types/Structs: `PascalCase`
- Constants: `UPPER_SNAKE_CASE`
- Module files: `lowercase_with_underscores`

### Comments

- Use `///` for public API documentation
- Use `//` for inline comments
- Write clear, descriptive comments

```rust
/// Scans directory for artifacts matching patterns
/// 
/// # Arguments
/// * `root` - Root directory to scan
/// * `patterns` - List of patterns to match
///
/// # Returns
/// Vec of found artifacts
pub fn scan(root: &Path, patterns: &[Pattern]) -> Result<Vec<Artifact>> {
    // Implementation
}
```

### Error Handling

Use `anyhow::Result` for public APIs:

```rust
pub fn do_something() -> Result<String> {
    let file = fs::read_to_string("path")
        .context("Failed to read file")?;
    Ok(file)
}
```

## Testing Guidelines

### Unit Tests

Place in the same file as the code being tested:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        let pattern = ArtifactPattern { /* ... */ };
        assert!(pattern.safe_to_delete);
    }
}
```

### Integration Tests

Create in `tests/` directory for end-to-end scenarios:

```rust
// tests/integration_test.rs
#[test]
fn test_end_to_end_scanning() {
    // Create temporary directory structure
    // Run klean
    // Verify results
}
```

### Testing Best Practices

- Use `tempfile` crate for test directories
- Clean up after tests
- Test both success and failure cases
- Use descriptive test names

## Performance Considerations

- Minimize allocations in hot paths
- Use iterators instead of collecting
- Leverage parallelism with rayon where appropriate
- Avoid excessive cloning

```rust
// ❌ Avoid
let items: Vec<_> = data.iter().clone().collect();

// ✅ Prefer
let items = data.iter();
```

## Documentation Standards

### Updating README

- Add new features to feature list
- Update command reference if adding flags
- Add examples for new functionality
- Keep consistent formatting

### Cargo Docs

Update doc comments for public APIs:

```bash
cargo doc --open
```

## Platform Support

### Target Platforms

- Linux (x86_64, ARM)
- macOS (x86_64, ARM64)
- Windows (x86_64)

Test your changes on multiple platforms if possible.

## Release Process

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Create PR with release notes
4. Tag release: `git tag v0.2.0`
5. Push tag: `git push origin v0.2.0`

## Getting Help

- 💬 Ask questions in [Discussions](https://github.com/danil0ws/klean/discussions)
- 📖 Check [Documentation](https://github.com/danil0ws/klean/wiki)
- 🐛 Search [Issues](https://github.com/danil0ws/klean/issues)

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)

## Recognition

Contributors will be:
- Listed in README.md
- Mentioned in release notes
- Credited in git history

Thank you for making klean better! 🎉
