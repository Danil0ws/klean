# klean

A safe, efficient CLI for cleaning development environments.

## Install

**Homebrew (macOS/Linux)**
```sh
brew tap klean-cli/klean
brew install klean
```

**Cargo**
```sh
cargo install klean
```

**Scoop (Windows)**
```sh
scoop bucket add klean https://github.com/klean-cli/scoop-bucket
scoop install klean
```

**Direct download** — grab the latest binary from [Releases](https://github.com/danil0ws/klean/releases).

## Usage

```
klean                   # interactive menu (TUI)
klean .                 # scan current directory
klean ~/projects        # scan a specific path
klean globals           # manage globally installed packages
klean update            # self-update to the latest version
klean undo              # restore the last deleted session from trash
klean trash             # list trash sessions
klean trash --empty     # permanently empty trash
klean stats             # show cleanup history
```

### Interactive menu

Running `klean` without arguments opens the TUI menu:

| # | Action |
|---|--------|
| 1 | Clean artifacts — remove build caches, node_modules, dist, … |
| 2 | Global packages — manage npm/cargo/pip/brew/binary installs |
| 3 | Statistics — cleanup history |
| 4 | Update *(shown only when a new version is available)* |
| 4/5 | Quit |

Navigate with `↑↓` or `jk`, confirm with `Enter`, or press the number key.

### Global packages

Press `u` to check for outdated packages (badges appear in the Version column).  
Press `Space` to select, `a` to select all, `Enter` to remove selected.  
Press `s` to toggle sort between manager and size.

## What gets cleaned

klean removes: `node_modules`, `target`, `.cache`, `dist`, `build`, `.next`,
`.nuxt`, `__pycache__`, `.venv`, `vendor`, and many more — for 20+ languages
and frameworks. See `klean.toml` for per-project configuration.

## Configuration (`klean.toml`)

Place a `klean.toml` at the root of a project to customise behaviour:

```toml
[project]
backup = true          # move to trash instead of deleting
backup_dir = ".trash"  # custom trash location

[[ignore]]
path = "dist"          # never touch this directory
```

## Self-update

```sh
klean update           # update to latest
klean update --dry-run # preview only
```

klean checks crates.io for the latest version and installs it via
`cargo install` if cargo is available, or downloads the pre-built binary
from GitHub Releases otherwise.

## License

MIT OR Apache-2.0
