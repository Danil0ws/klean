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

    /// Respect .gitignore patterns (enabled by default)
    #[arg(long)]
    pub respect_gitignore: bool,

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
}

impl Cli {
    pub fn get_path(&self) -> PathBuf {
        self.path.clone().unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn should_respect_gitignore(&self) -> bool {
        self.respect_gitignore || !self.respect_gitignore // Default is true
    }
}
