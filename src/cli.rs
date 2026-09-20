use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "klean")]
#[command(about = "A safe, efficient CLI for cleaning development environments", long_about = None)]
#[command(version)]
#[command(author)]
pub struct Cli {
    /// Directory to scan for artifacts (defaults to current directory)
    #[arg(short, long, value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Only list what would be removed, don't actually remove
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Skip confirmation and remove immediately
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Filter by specific pattern (e.g., "node_modules", "target")
    #[arg(short, long, value_name = "PATTERN")]
    pub filter: Option<String>,

    /// Minimum size to consider for removal (e.g., "100MB", "1GB")
    #[arg(long, value_name = "SIZE")]
    pub min_size: Option<String>,

    /// Maximum size to consider for removal (e.g., "500MB")
    #[arg(long, value_name = "SIZE")]
    pub max_size: Option<String>,

    /// Custom .klignore file to use
    #[arg(long, value_name = "PATH")]
    pub klignore: Option<PathBuf>,

    /// Do not use .gitignore to prune traversal (respecting it is the default)
    #[arg(long)]
    pub no_gitignore: bool,

    /// Allow scanning/cleaning in sensitive system paths (disabled by default)
    #[arg(long)]
    pub allow_system_paths: bool,

    /// Directory to move items to instead of deleting (backup)
    #[arg(long, value_name = "PATH")]
    pub backup_dir: Option<PathBuf>,

    /// Operation mode
    #[arg(value_enum, default_value = "interactive")]
    pub mode: Option<Mode>,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress all output
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show configuration and exit
    #[arg(long)]
    pub show_config: bool,

    /// Machine-readable JSON on stdout (scan document, or stats with `stats`)
    #[arg(long)]
    pub json: bool,

    /// Exit with code 2 when the total to clean exceeds SIZE (CI gate, e.g. "2GB")
    #[arg(long, value_name = "SIZE")]
    pub fail_if_over: Option<String>,

    /// `serve`: address to bind (default 127.0.0.1)
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// `serve`: port to bind
    #[arg(long, value_name = "PORT", default_value_t = 8731)]
    pub port: u16,

    /// `serve`: bearer token required by /api/* when binding a non-loopback host
    /// (KLEAN_TOKEN is used when the flag is absent)
    #[arg(long, value_name = "TOKEN")]
    pub token: Option<String>,

    /// `watch`: rescan interval (30s, 15m, 2h, 1d)
    #[arg(long, value_name = "DURATION", default_value = "1h")]
    pub interval: String,

    /// `watch`: delete matching artifacts instead of only reporting them
    #[arg(long)]
    pub auto_clean: bool,

    /// `watch`: run a single pass and exit (cron/CI friendly)
    #[arg(long)]
    pub once: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum Mode {
    /// Interactive mode with TUI (default)
    #[value(name = "interactive")]
    Interactive,
    /// Non-interactive CLI mode
    #[value(name = "cli")]
    Cli,
    /// Only list artifacts without removing
    #[value(name = "list")]
    List,
    /// Web UI + JSON API for managing a machine remotely
    #[value(name = "serve")]
    Serve,
    /// Space saved over time (from the local history file)
    #[value(name = "stats")]
    Stats,
    /// Rescan on an interval, optionally cleaning automatically
    #[value(name = "watch")]
    Watch,
    /// List the pattern plugins that are loaded
    #[value(name = "plugins")]
    Plugins,
}

impl Cli {
    pub fn get_path(&self) -> PathBuf {
        self.path.clone().unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn should_respect_gitignore(&self) -> bool {
        !self.no_gitignore
    }
}
