# klean 🧹

A safe, efficient, and extensible CLI tool for cleaning development environments across all programming languages.

Inspired by [npkill](https://github.com/voidcosmos/npkill), **klean** expands the concept to cover all types of development artifacts: `node_modules`, Python virtual environments, Rust build directories, Java dependencies, and much more.

## Features

### 🎯 Intelligent Multi-Language Scanning
- Detects cleanup-safe directories for 20+ programming languages and frameworks
- Supports Node.js, Python, Rust, Java, C#, C++, PHP, Ruby, Go, and more
- Automatically identifies project type by markers (package.json, Cargo.toml, pom.xml, etc.)

### 🔐 Safety-First Design
- Never deletes system or project files
- Requires explicit .klignore rules to protect directories
- Dry-run mode to preview what would be deleted
- Respects .gitignore patterns by default
- Validates artifacts before deletion

### 🎪 Interactive & Non-Interactive Modes
- **Interactive TUI**: Navigate, select, and confirm with visual feedback
- **CLI Mode**: Automate cleaning with flags
- **List Mode**: View artifacts without modifying

### ⚙️ Highly Configurable
- `.klignore` files for custom protection rules (`.gitignore` format)
- `klean.toml` for project-specific and global settings
- Custom artifact patterns
- Backup directory support (move instead of delete)
- Size-based filtering

### 📊 Performance Optimized
- Parallel directory scanning with rayon
- Efficient size calculation
- Progress bars for long operations

## Installation

### Quick Install

**macOS (Homebrew)**
```bash
brew tap klean/klean
brew install klean
```

**Linux (Ubuntu/Debian)**
```bash
curl -s https://packagecloud.io/install/repositories/klean/klean/script.deb.sh | sudo bash
sudo apt-get install klean
```

**Arch Linux**
```bash
yay -S klean
# or
git clone https://aur.archlinux.org/klean.git && cd klean && makepkg -si
```

**Windows (Scoop)**
```powershell
scoop bucket add klean https://github.com/danil0ws/scoop-bucket
scoop install klean
```

**Windows (Chocolatey)**
```powershell
choco install klean
```

**Windows (winget)**
```powershell
winget install klean.klean
```

**Cross-platform (Mise/asdf)**
```bash
mise plugin add klean https://github.com/danil0ws/mise-klean
mise install klean@latest
```

**From Source**
```bash
git clone https://github.com/danil0ws/klean.git
cd klean
cargo install --path .
```

**From Cargo**
```bash
cargo install klean
```

For detailed installation instructions and troubleshooting, see [INSTALLATION.md](INSTALLATION.md).

### Pre-built Binaries

Download from [releases page](https://github.com/danil0ws/klean/releases)

## Quick Start

### Interactive Mode (Default)

```bash
# Start interactive mode in current directory
klean

# Start in a specific directory
klean --path ~/projects
```

**Keyboard Controls:**
- `↑↓` - Navigate
- `Space` - Select/deselect
- `A` - Select all
- `D` - Deselect all
- `Enter` - Clean selected
- `Q` - Quit

### Dry Run (Preview)

```bash
# See what would be deleted without actually deleting
klean --dry-run
```

### CLI Mode

```bash
# Delete all node_modules without confirmation
klean --filter "node_modules" --yes

# Delete only Python cache, preview first
klean --filter "pycache" --dry-run

# Delete directories larger than 100MB
klean --min-size 100MB --yes

# Delete with backup instead of permanent deletion
klean --backup-dir ~/.klean-backups --yes
```

### Size Filtering

```bash
# Delete artifacts between 50MB and 500MB
klean --min-size 50MB --max-size 500MB

# Size units: B, KB, MB, GB (case-insensitive)
klean --min-size 1.5GB
```

## Configuration

### .klignore Format

Create a `.klignore` file in your project root (or `~/.klignore` for global rules):

```bash
# Protect specific directories (similar to .gitignore)
projetos-importantes/meu-app/node_modules

# Protect from root
/.venv

# Wildcard patterns
apps/*/node_modules
src/**/build

# Comments are supported
# This is a comment

# Blank lines are ignored (as shown above)
```

### klean.toml

Project-specific or global configuration:

```toml
# ~/.config/klean/config.toml (global)
# or ./klean.toml (project-specific)

# Backup directory instead of deleting
backup_dir = "/home/user/.klean-backups"

# Respect .gitignore patterns
respect_gitignore = true

# Custom artifact patterns
[[patterns]]
name = "my_build_cache"
patterns = [".my_cache", "build_output"]
languages = ["MyLanguage"]
description = "My custom build cache"
safe_to_delete = true
```

## Usage Examples

### Find and clean old Python environments

```bash
# List all Python virtual environments
klean --filter "python-venv" --dry-run

# Clean with backup
klean --filter "python-venv" --backup-dir ~/old-envs --yes
```

### Clean Rust projects over 500MB

```bash
klean --filter "rust-target" --min-size 500MB --yes
```

### Selective cleanup with confirmation

```bash
# In interactive mode, you can select specific artifacts
klean --path ~/projects/monorepo

# Then press: A (select all) or Space (select individual items)
# And press: Enter to proceed with confirmation
```

### Verbose output for debugging

```bash
# Show what's happening
klean -vv

# Triple verbose for detailed trace
klean -vvv

# Suppress output
klean -q
```

## Supported Artifacts

### Node.js/JavaScript
- `node_modules`
- `.npm`
- `.next`
- `.nuxt`
- `dist`

### Python
- `__pycache__`
- `.venv`, `venv`, `env`, `.tox`
- `*.egg-info`, `dist`, `build`

### Rust
- `target`

### Java/Kotlin/Gradle
- `build`
- `.gradle`
- `target`

### C#/.NET
- `bin`
- `obj`
- `.vs`

### PHP/Ruby
- `vendor`
- `.bundle`
- `vendor/bundle`

### Elixir
- `_build`
- `deps`

### Haskell
- `.stack-work`
- `dist-newstyle`

### Mobile
- `.android`
- `.ios`
- `DerivedData`

### IDE & Editors
- `.vscode/extensions`

And many more! Add custom patterns in `klean.toml`.

## Command Reference

```
USAGE:
    klean [OPTIONS] [MODE]

OPTIONS:
    -p, --path <PATH>                 Directory to scan (default: current)
    -n, --dry-run                     List only, don't delete
    -y, --yes                         Skip confirmation
    --filter <PATTERN>                Filter by pattern (e.g., "node_modules")
    --min-size <SIZE>                 Minimum size (e.g., "100MB")
    --max-size <SIZE>                 Maximum size (e.g., "500MB")
    --klignore <PATH>                 Custom .klignore file
    --respect-gitignore               Respect .gitignore (default: true)
    --backup-dir <PATH>               Move to backup dir instead of deleting
    -v, --verbose                     Increase verbosity
    -q, --quiet                       Suppress output
    --show-config                     Show configuration and exit
    -h, --help                        Print help
    -V, --version                     Print version

MODES:
    interactive                       Interactive TUI (default)
    cli                              Non-interactive CLI
    list                             List artifacts only
```

## Performance

On a typical development machine:

- Scanning 500+ directories: ~500ms
- Calculating sizes: ~1-2s
- Interactive UI: Instant response

Performance scales well with SSD storage and decreases on slower storage.

## Security & Safety

1. **Artifact Validation**: Only deletes known artifact directories
2. **Marker Files**: Verifies parent directory contains project files (package.json, Cargo.toml, etc.)
3. **Ignore Rules**: .klignore files prevent accidental deletion
4. **Confirmation**: Interactive mode always confirms before deletion
5. **Dry Run**: Preview exactly what will be deleted
6. **Backup Option**: Move instead of permanently deleting

## Development

### Project Structure

```
klean/
├── src/
│   ├── main.rs          # Entry point
│   ├── cli.rs           # CLI argument parsing
│   ├── patterns.rs      # Artifact pattern definitions
│   ├── ignore.rs        # .klignore/.gitignore parsing
│   ├── scanner.rs       # Directory scanning logic
│   ├── cleaner.rs       # Safe deletion logic
│   ├── config.rs        # Configuration loading
│   └── tui/
│       ├── mod.rs       # TUI components
│       └── interactive.rs # Interactive mode
├── Cargo.toml
├── README.md
├── CONTRIBUTING.md
└── LICENSE
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Generate documentation
cargo doc --open
```

### Code Quality

```bash
# Check code style
cargo clippy

# Format code
cargo fmt
```

## CI/CD and Release Process

The klean project uses GitHub Actions for automated building, testing, and releasing to multiple package managers.

### Automated Release Process

When you push a semantic version tag (e.g., `v1.0.0`), the following happens automatically:

1. **Build & Test**
   - Compiles for Linux (x86_64, aarch64, musl)
   - Compiles for macOS (x86_64, aarch64)
   - Compiles for Windows (x86_64)
   - Runs all tests

2. **GitHub Release**
   - Creates GitHub Release with all binaries
   - Generates SHA256 checksums
   - Publishes to crates.io

3. **Package Manager Updates** (parallel)
   - Updates Homebrew tap
   - Updates Scoop bucket
   - Publishes to Chocolatey
   - Updates Arch User Repository (AUR)
   - Builds .deb and .rpm packages
   - Updates Mise plugin

### Triggering a Release

```bash
# Tag a new version
git tag v1.0.0
git push origin v1.0.0

# GitHub Actions automatically builds and publishes
# Monitor progress at: https://github.com/danil0ws/klean/actions
```

### Workflows

All CI/CD workflows are in `.github/workflows/`:

- **build-release.yml** - Main build and release workflow
- **homebrew-update.yml** - Updates Homebrew formula
- **scoop-update.yml** - Updates Scoop bucket
- **linux-packages.yml** - Builds deb/rpm packages
- **aur-update.yml** - Updates AUR package
- **windows-packages.yml** - Updates Windows package managers

For setup and configuration details, see [PACKAGE_MANAGERS.md](PACKAGE_MANAGERS.md).

### Composite Actions

Reusable build action at `.github/actions/build-binary/action.yml` for consistent builds across workflows.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Roadmap

- [ ] Plugin system for custom artifact types
- [ ] Statistics and reporting (space saved over time)
- [ ] Web UI for remote server management
- [ ] Cross-platform binary distribution
- [ ] Integration with CI/CD pipelines
- [ ] Watch mode (auto-clean on schedule)

## Troubleshooting

### Nothing is found

1. Check your current directory: `pwd`
2. Try specifying a path: `klean --path ~/projects`
3. Check .klignore rules: `cat .klignore`
4. Verify patterns exist: `klean --show-config`

### "Permission denied" errors

Make sure you have write permissions to the directories being cleaned:

```bash
# Check permissions
ls -la target/
ls -la node_modules/

# Run with appropriate permissions if needed
sudo klean  # Use with caution!
```

### Artifacts not being detected

1. Check if they match a known pattern: `klean --show-config`
2. Add custom pattern to `klean.toml`
3. Verify with: `klean --dry-run --verbose`

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Inspired by [npkill](https://github.com/voidcosmos/npkill)
- Built with [Rust](https://www.rust-lang.org/)
- UI powered by [ratatui](https://github.com/ratatui/ratatui)
- CLI parsing with [clap](https://github.com/clap-rs/clap)

## Support

- 📖 [Documentation](https://github.com/danil0ws/klean/wiki)
- 🐛 [Report Issues](https://github.com/danil0ws/klean/issues)
- 💬 [Discussions](https://github.com/danil0ws/klean/discussions)
