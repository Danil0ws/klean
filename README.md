# klean 🧹

A safe, efficient, and extensible CLI tool for cleaning development environments across all programming languages.

Inspired by [npkill](https://github.com/voidcosmos/npkill), **klean** expands the concept to cover all types of development artifacts: `node_modules`, Python virtual environments, Rust build directories, Java dependencies, and much more.

## Features

### 🎯 Intelligent Multi-Language Scanning
- Detects cleanup-safe directories for 20+ programming languages and frameworks:
  49 pattern groups covering ~150 directory names, from `node_modules` and
  `target` to `.turbo`, `.mypy_cache`, `Library` (Unity) and `.terraform`
- Supports Node.js, Python, Rust, Java/Kotlin/Scala, .NET, C/C++, PHP, Ruby,
  Elixir, Haskell, Zig, Nim, Flutter, Godot, Terraform and more
- Identifies the owning project through marker files (package.json, Cargo.toml, pom.xml, ...)

### 🧱 One Directory per Artifact
- Reports each matched directory once and never descends into it, so results are
  disjoint: no `node_modules/pkg/node_modules`, no `target/debug/build`
- Results are grouped and ordered by project, largest project first

### 🗂️ Project-Level Control
- Deletion is managed per project: enable/disable a project or allow/deny specific
  artifacts inside it from `klean.toml`
- Artifacts blocked by a rule are reported as skipped instead of silently dropped

### 🔐 Safety-First Design
- Never deletes system or project files
- `.klignore` is an explicit protection list and always wins over patterns
- Refuses overlapping (nested) targets before touching the disk
- Dry-run mode to preview what would be deleted
- Uses `.gitignore` to prune traversal (not as protection)
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
- One deterministic pass over the tree (sorted `read_dir`, no re-walking a match)
- Artifacts are matched once and never descended into, so results stay disjoint
- Iterative size calculation (no recursion depth surprises)
- Progress bar while cleaning, JSON output and a web API for automation

## Installation

### Quick Install

**macOS (Homebrew)**
```bash
brew tap danil0ws/klean
brew install klean
```

**Linux (Ubuntu/Debian)**
```bash
curl -s https://packagecloud.io/install/repositories/danil0ws/klean/script.deb.sh | sudo bash
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

**Cross-platform (mise/asdf)**
```bash
mise plugin add klean https://github.com/danil0ws/mise-klean
mise install klean@latest

# asdf works with the same plugin
asdf plugin add klean https://github.com/danil0ws/mise-klean
asdf install klean latest
```

**From Source**
```bash
git clone https://github.com/danil0ws/klean.git
cd klean
cargo install --path .
```

**From Git (works today)**
```bash
cargo install --git https://github.com/danil0ws/klean --locked
```

**From crates.io** — only after the first publish. The crate is not on the
registry yet, so today `cargo install klean` fails with
`could not find 'klean' in registry 'crates-io'`:
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
- `↑↓` / mouse wheel - Navigate one row
- `PgUp` / `PgDn` - Move one screen
- `Home` / `End` - Jump to first/last
- `Space` - Select/deselect
- `A` - Select all
- `D` - Deselect all
- `Enter` / `Y` - Clean selected (confirms, then shows the summary in place)
- `T` - (in the confirmation) move the selection to the trash instead — undoable
- `U` - (on the summary) undo the trashed run: everything comes back
- `Q` - Quit

The UI does not close after cleaning: the summary stays on screen, the deleted
rows leave the list, the freed total accumulates in the footer, and `Q` exits
when you are done. While it scans, the screen shows the logo, the folder being
read, how many folders were walked and how long it has been running.

### Undo

Deleting is not reversible, so the confirmation gives you a second option.

```bash
# interactive: Enter/Y deletes, T moves to the klean trash, U on the summary
# brings it back. From the shell, after the fact:
klean undo              # restore the last trashed session
klean trash             # what an undo can still bring back
klean trash --empty     # free that space now
klean --trash --yes     # scripted clean that stays undoable
```

The trash lives in `<config>/klean/trash` (override with `KLEAN_TRASH`).
Sessions older than 7 days are purged on the next trashed run, so the space is
reclaimed on its own; until then the items are still on disk.

Every release also carries a single version-bump command:
`scripts/bump-version.sh <x.y.z>` rewrites `Cargo.toml`, `Cargo.lock`, the
package-manager manifests, the installers and the docs; `--check` fails when a
copy drifted (CI runs it).

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

A `.klignore` is a **protection list**: anything it matches is never reported,
whatever the patterns say. Patterns resolve against the **scan root** (the
`--path` you passed, canonicalised) — not against the directory holding the file
and not against your current directory. `/node_modules` means
`<scan-root>/node_modules`, a bare `node_modules` matches at any depth, and a
leading `!` re-includes something an earlier line protected.

### klean.toml

Project-specific or global configuration. The two files have different names on
purpose: `<root>/klean.toml` travels with the repository, while
`~/.config/klean/config.toml` is your machine's default. They merge additively
(global first, then project) and the most specific `[[projects]]` rule wins.

```toml
# ~/.config/klean/config.toml (global)
# or ./klean.toml (project-specific)

# Backup directory instead of deleting
backup_dir = "/home/user/.klean-backups"

# Use .gitignore to prune traversal (default: true)
respect_gitignore = true

# Custom artifact patterns
[[patterns]]
name = "my_build_cache"
patterns = [".my_cache", "build_output"]
languages = ["MyLanguage"]
description = "My custom build cache"
safe_to_delete = true

# Per-project deletion policy. The most specific matching rule wins; anything
# blocked by a rule is reported as skipped and never deleted.
[[projects]]
path = "~/Work/production-app"   # absolute, ~/-prefixed or glob
enabled = false                  # never delete anything here

[[projects]]
path = "~/Dev/monorepo"
allow = ["node_modules", "dist", ".next"]  # only these may be deleted
deny = ["target"]                          # never these
```

Artifacts are attributed to the nearest ancestor directory containing a marker
file (`package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`, `.git`, ...).
Rules match against that project root, so a rule for a monorepo covers every
package inside it unless a more specific rule (longer path) exists.

### How results are presented

```
$ klean -p ~/Dev list
📋 Artifacts by project (largest first):
------------------------------------------------------------------------------

▸ buff  (5.1 GiB, 3 items)
    node_modules              1.9 GiB  buff/apps/painel-elo-next/node_modules
    .next                   900.1 MiB  buff/apps/painel-elo-next/.next
    node_modules            612.0 MiB  buff/apps/agent/node_modules

▸ voz-amiga  (820.4 MiB, 1 item)
    node_modules            820.4 MiB  voz-amiga/services/gateway/node_modules
------------------------------------------------------------------------------
```

No entry is ever a child of another entry, so a project's total is honest and
deleting the whole list can never fail half-way because of a missing directory.

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

Matched by directory name (exact or `*`/`?` glob), described per ecosystem.
Entries marked ⚠ are still cleanable but flagged as "possibly wanted" because
the name can also mean something else in a given repo.

### JavaScript / TypeScript
- `node_modules`, `bower_components`, `.npm`, `.pnpm-store`
- `.next`, `.nuxt`, `.output`, `.svelte-kit`, `.astro`, `.angular`, `.docusaurus`
- `.vite`, `.turbo`, `.parcel-cache`, `.webpack`, `.rollup.cache`
- `storybook-static`, `.expo`, `.vercel`, `.netlify`, `.serverless`, `.aws-sam`
- `dist`, `coverage`, `.nyc_output`

### Python
- `__pycache__`, `*.egg-info`, `dist`, `build`, `develop-eggs`
- `.venv`, `venv`, `env`, `.tox`, `.nox`
- `.mypy_cache`, `.ruff_cache`, `.pytest_cache`, `.pytype`, `.pyre`, `.hypothesis`
- `htmlcov`, `.ipynb_checkpoints`, `__pypackages__`, `.eggs`, `pip-wheel-metadata`

### Rust / Go
- `target`

### JVM (Java, Kotlin, Scala, Clojure, Groovy)
- `build`, `.gradle`, `.kotlin`, `target`
- `out` ⚠ (IDE compiler output)
- `.scala-build`, `.bloop`, `.metals`, `.bsp`, `.cpcache`, `.clj-kondo`

### .NET (C#, F#, VB)
- `obj`, `.vs`, `TestResults`, `BenchmarkDotNet.Artifacts`

### C / C++ / Objective-C
- `CMakeFiles`, `cmake-build-debug`, `cmake-build-release`, `_deps`, `builddir`
- `autom4te.cache`, `.deps`, `.libs`

### Apple (Swift, Objective-C)
- `DerivedData`, `Pods`, `Carthage`, `.build`, `Build`, `xcuserdata`

### Android / Dart / Flutter
- `.cxx`, `.externalNativeBuild`, `captures`
- `.dart_tool`, `.pub-cache`

### Game engines
- `Library` ⚠, `Temp` ⚠, `Logs` ⚠, `Builds` ⚠ (Unity)
- `.godot`, `.import`, `.mono` (Godot)

### BEAM / functional
- `_build`, `deps`, `.elixir_ls`, `cover` (Elixir, Erlang)
- `_opam` (OCaml), `.stack-work`, `dist-newstyle` (Haskell)
- `zig-cache`, `zig-out`, `.zig-cache` (Zig), `nimcache` (Nim)
- `output` ⚠ (PureScript)

### Ruby / PHP
- `.bundle`, `.yardoc`
- `vendor` (Composer), `.phpunit.cache`, `.php-cs-fixer.cache`, `.phpstan-cache`

### Infrastructure / experiments
- `.terraform`, `.terragrunt-cache`
- `mlruns` ⚠, `wandb` ⚠, `lightning_logs` ⚠, `.lightning` ⚠ (ML runs — keep if
  they are not reproducible)

### Generic
- `dist`, `coverage`, `.cache`, `.idea` ⚠, `.sass-cache`

Deliberately **not** matched: `log`, `logs`, `tmp`, `lib`, `bin`, `packages` and
`vendor` for Go — names as likely to be hand-written source as build output
(`bin` matched a real mise plugin and `packages` the pnpm workspace of a real
project on this machine, so both were dropped). Add them per project with
`[[patterns]]` in `klean.toml` or with a plugin when you know the layout.

And many more! Add custom patterns in `klean.toml`.

## Plugins, web UI, watch mode and CI

### Plugins (custom artifact types)

Drop a TOML file in `~/.config/klean/plugins/` (global) or `./.klean/plugins/`
(per project). Either a single pattern table or several under `[[patterns]]`:

```toml
[[patterns]]
name = "blender-cache"
patterns = ["blender_cache", "*.blend1"]
languages = ["Generic"]
description = "Blender temporary render cache"
safe_to_delete = true
```

```bash
klean plugins        # which dirs are searched and what loaded
```

A plugin that fails to parse is reported and skipped; it never stops a scan.

### Web UI (remote management)

```bash
klean --path ~/work serve                      # http://127.0.0.1:8731
klean --path ~/work serve --host 0.0.0.0 --token $KLEAN_TOKEN
```

`GET /` serves a single-page UI (select + clean), `/api/scan` returns the same
JSON as `--json`, and `POST /api/clean` accepts `{"paths": [...]}`. Binding a
non-loopback address **requires** a token, the API only deletes paths that a
fresh scan reports, and `KLEAN_TOKEN` is read when `--token` is absent.

### Watch mode

```bash
klean --path ~/work watch --interval 30m              # report only
klean --path ~/work watch --interval 30m --auto-clean # delete (respects rules)
klean --path ~/work watch --once --auto-clean         # one pass, for cron
klean --path ~/work watch --once --fail-if-over 5GB   # cron that fails (exit 2)
```

Project rules (`[[projects]]`) and `.klignore` apply: anything blocked is
reported, never deleted. `--fail-if-over` works in `watch --once` too, so a cron
job can alert on a budget breach without parsing the log.

### Statistics

```bash
klean stats           # total freed, per project, last runs
klean stats --json    # same data for scripts/dashboards
```

Every real cleaning run appends one line to `~/.config/klean/history.tsv`
(override with `KLEAN_HISTORY`). Dry runs are not recorded.

### CI/CD

```bash
klean --path . --json list                    # machine-readable scan
klean --path . --fail-if-over 2GB list        # exits 2 when over budget
```

Or use the composite action:

```yaml
- uses: danil0ws/klean/.github/actions/klean@main
  with:
    path: .
    max-size: 2GB
```

See `examples/ci/klean-budget.yml` for a complete workflow.

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
    --no-gitignore                    Don't use .gitignore to prune traversal
    --allow-system-paths              Allow scanning sensitive system paths
    --backup-dir <PATH>               Move to backup dir instead of deleting
    --trash                           Move to the klean trash (`klean undo` restores)
    --empty                           trash: purge it now instead of after 7 days
    --json                            Machine-readable JSON on stdout
    --fail-if-over <SIZE>             Exit 2 when the total exceeds SIZE
    --host <HOST>                     serve: address to bind (default 127.0.0.1)
    --port <PORT>                     serve: port (default 8731)
    --token <TOKEN>                   serve: bearer token for /api/* (KLEAN_TOKEN)
    --interval <DURATION>             watch: 30s, 15m, 2h, 1d (default 1h)
    --auto-clean                      watch: delete instead of only reporting
    --once                            watch: single pass and exit (cron/CI)
    -v, --verbose                     Increase verbosity
    -q, --quiet                       Suppress output
    --show-config                     Show configuration and exit
    -h, --help                        Print help
    -V, --version                     Print version

MODES:
    interactive                       Interactive TUI (default)
    cli                               Non-interactive CLI
    list                              List artifacts only
    serve                             Web UI + JSON API
    stats                             Space freed over time
    watch                             Rescan on an interval
    plugins                           List loaded pattern plugins
    undo                              Restore the last trashed items
    trash                             Show the trash (with `--empty`, purge it)

EXIT CODES:
    0   success
    1   error
    2   --fail-if-over exceeded
```

## Performance

Measured on the machine this repository is developed on (macOS Intel, SSD,
release build):

| tree | artifacts | total | `klean list` |
| ---- | --------- | ----- | ------------ |
| `~/Dev/klean` (this repo) | 1 | 0.1 GiB | 0.4 s |
| `~/Dev` (23 projects) | 69 | 7.9 GiB | 14.6 s |
| `~` (whole home) | — | — | 135 s |

The walk is not what costs: `calculate_dir_size` reads every file inside every
artifact (`node_modules`, `target`, `.venv` hold hundreds of thousands of them)
and it is single threaded, so time tracks the *content* of the artifacts, not the
number of directories. Cleaning one `node_modules` stays interactive; scanning a
whole home directory is a coffee break. Narrow the scan with `--filter`,
`--min-size`/`--max-size` or `[[projects]]` when you only care about part of it.

## Security & Safety

1. **Artifact Validation**: Only deletes known artifact directories
2. **Marker Files**: Verifies parent directory contains project files (package.json, Cargo.toml, etc.)
3. **Ignore Rules**: .klignore files prevent accidental deletion
4. **Confirmation**: Interactive mode always confirms before deletion
5. **Dry Run**: Preview exactly what will be deleted
6. **Backup Option**: Move instead of permanently deleting
7. **Undo**: `T` in the confirmation (or `--trash`) moves items to
   `<config>/klean/trash` and `klean undo` brings them back
8. **Symlinks are neither followed nor reported**: a `node_modules` that is a
   symlink (pnpm workspaces, Nix, shared caches) is skipped, and a directory that
   is itself a link is never a target — klean deletes the real directory it
   matched, so nothing outside the scan tree can be removed through a link.
   Point `--path` at the real location when a link hides an artifact you wanted —
   `klean -p <real-dir> list` shows it.
9. **Sensitive paths**: `/`, `/etc`, `/usr`, `/var`, `/tmp`, `/System` and
   friends are skipped unless the scan root is already inside them, and the
   cleaner re-checks before deleting.

## Development

### Project Structure

```
klean/
├── src/
│   ├── main.rs          # Entry point, mode dispatch, JSON output
│   ├── lib.rs           # Library surface (so tests can use the modules)
│   ├── cli.rs           # CLI argument parsing and modes
│   ├── patterns.rs      # Built-in artifact patterns + glob matching
│   ├── plugins.rs       # Pattern plugins from *.toml plugin dirs
│   ├── ignore.rs        # .klignore (protection) / .gitignore (pruning)
│   ├── scanner.rs       # Non-recursive scanning, project attribution
│   ├── config.rs        # klean.toml loading and per-project rules
│   ├── cleaner.rs       # Safe deletion/backup/trash and safety checks
│   ├── trash.rs         # Klean trash + `klean undo` (journal per session)
│   ├── history.rs       # Space-saved history (klean stats)
│   ├── watch.rs         # Interval rescan / auto-clean
│   ├── web.rs           # Web UI + JSON API (klean serve)
│   └── tui/
│       ├── mod.rs       # Table layout, status bar, confirmation dialog
│       └── interactive.rs # Interactive mode
├── scripts/
│   ├── install.sh       # curl | sh installer (checksum verified)
│   ├── install.ps1      # PowerShell installer
│   ├── check-packages.py # Validates generated package files in CI
│   └── bump-version.sh  # One-command version bump (`--check` guards CI)
├── examples/
│   ├── package-managers/ # Homebrew/Scoop/Chocolatey/AUR/mise templates
│   └── ci/klean-budget.yml # Ready-made CI job with a size budget
├── .github/
│   ├── workflows/       # ci.yml, build-release.yml, per-manager workflows
│   └── actions/klean/   # Composite action for other repositories
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

- **ci.yml** - Format, clippy (`-D warnings`), tests (Linux/macOS/Windows), release build
  and a packaging-template check
- **build-release.yml** - On a `v*` tag: builds every target, creates the GitHub
  Release with SHA256SUMS, publishes to crates.io, then calls the package-manager
  workflows below as reusable workflows
- **homebrew-update.yml** - Regenerates the formula (both macOS arches) and installs
  it from the tap on a macOS runner
- **scoop-update.yml** - Regenerates `bucket/klean.json` and installs it on Windows
- **windows-packages.yml** - Chocolatey (pack + push), winget (manifest PR) and the
  mise/asdf plugin
- **linux-packages.yml** - Builds `.deb`, `.rpm` and the Arch `PKGBUILD`, uploads them
  to the release and (optionally) Packagecloud
- **aur-update.yml** - Pushes `PKGBUILD` + `.SRCINFO` to the AUR
- **docker.yml** - Builds and pushes the multi-arch container image

Package publication only needs repository secrets, never manual steps; see
[PACKAGE_MANAGERS.md](PACKAGE_MANAGERS.md) for the repo/secret setup,
[CI_CD.md](CI_CD.md) for the full pipeline (jobs, gates, failure playbook), and
`scripts/check-packages.py` to validate the generated package files locally
(`python3 scripts/check-packages.py`).


### Composite Actions

Reusable build action at `.github/actions/build-binary/action.yml` for consistent builds across workflows.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Roadmap

The original six items all shipped:

- [x] **Plugin system for custom artifact types** — drop a `.toml` in
  `~/.config/klean/plugins/` (global) or `./.klean/plugins/` (per project); list
  what loaded with `klean plugins`
- [x] **Statistics and reporting** — `klean stats` (space freed over time, per
  project), stored as TSV in `~/.config/klean/history.tsv`
- [x] **Web UI for remote server management** — `klean serve` with
  `--host/--port/--token`; JSON API at `/api/scan` and `/api/clean`
- [x] **Cross-platform binary distribution** — prebuilt release binaries plus
  `scripts/install.sh` (Linux/macOS) and `scripts/install.ps1` (Windows), both
  verifying the release `SHA256SUMS`
- [x] **Integration with CI/CD pipelines** — `--json` output, `--fail-if-over`
  (exits `2`) and the `klean` composite action
  (`.github/actions/klean`), see `examples/ci/klean-budget.yml`
- [x] **Watch mode (auto-clean on schedule)** — `klean watch --interval 30m`
  (report) or `--auto-clean` (act), `--once` for cron/CI

Shipped after that list: undo (`--trash`, `klean undo`, `klean trash`), the
catalogue expansion to 49 pattern groups, the scanning loading screen, and mouse
wheel/paging in the UI.

Ideas after that: per-project retention rules, a `klean doctor` (and
`klean explain <path>` for "why was this not detected?"), shell completions and
man pages, `--trash` everywhere a delete happens, and metrics export
(Prometheus/OpenTelemetry) on top of the history file. The measured numbers in
[Performance](#performance) name the other known gap: size calculation is still
single threaded.

## Troubleshooting

### Nothing is found

1. Check your current directory: `pwd`
2. Try specifying a path: `klean --path ~/projects`
3. Check .klignore rules: `cat .klignore` — it is a **protection** list, so
   anything it matches is invisible on purpose (anchors resolve against the scan
   root, not the file or your cwd)
4. Verify patterns exist: `klean --show-config`
5. Is the artifact a symlink? Links are not followed, so a linked `node_modules`
   is skipped: point `--path` at the real directory
6. Running an old binary? An installed release can be behind the catalogue:
   `klean --version`, then `cargo install --path . --locked --force`

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
