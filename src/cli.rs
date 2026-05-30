use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tlink", about = "tmux:// deeplink CLI for macOS", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Interactive TUI wizard: select terminal, register tmux:// URI scheme
    Setup,
    /// Handle a tmux:// URI (invoked by the OS when a deeplink is clicked)
    Open {
        /// The tmux:// URI, e.g. tmux://mysession/0/1
        uri: String,
    },
    /// Show URI scheme registration status and active tmux sessions
    Status,
    /// Re-register the URI scheme handler without re-running setup
    Restart,
    /// Run diagnostic checks and report pass/fail
    Doctor,
    /// Install a tlink add-on
    Install {
        /// Add-on name (e.g. claude-notification)
        addon: Option<String>,

        /// Interactively select add-ons to install
        #[arg(short = 'i', long = "interactive", conflicts_with = "addon")]
        interactive: bool,
    },
    /// Remove a tlink add-on
    Delete {
        /// Add-on name (e.g. claude-notification)
        addon: String,
    },
    /// List available add-ons
    List {
        #[command(subcommand)]
        target: ListTarget,
    },
    /// Manage telemetry preferences
    Telemetry {
        #[command(subcommand)]
        action: TelemetryAction,
    },
    /// Fire a desktop notification from a Claude Code hook (reads JSON from stdin)
    #[command(hide = true)]
    Notify {
        #[arg(long)]
        session: String,
        #[arg(long)]
        window: String,
        #[arg(long)]
        pane: String,
        /// Terminal emulator detected from tmux client_termtype (e.g. "ghostty 1.2.3")
        #[arg(long, default_value = "")]
        term: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TelemetryAction {
    /// Enable telemetry (anonymous usage data + optional error tracking)
    Enable {
        /// Sentry DSN for error tracking (optional)
        #[arg(long, env = "TLINK_SENTRY_DSN")]
        dsn: Option<String>,
    },
    /// Disable telemetry
    Disable,
    /// Show current telemetry status
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ListTarget {
    /// Show all add-ons and their status
    #[command(name = "add-ons")]
    Addons,
}
