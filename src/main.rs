mod addon;
mod bundle;
mod cli;
mod config;
mod doctor;
mod notify;
mod open;
mod restart;
mod setup;
mod status;
mod telemetry;
mod terminal;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ListTarget, TelemetryAction};

fn main() {
    telemetry::init();
    let result = run();
    // Record event _before_ shutdown so Sentry can flush it
    if let Err(ref e) = result {
        telemetry::record_event(
            "command.fail",
            Some(serde_json::json!({
                "error": format!("{e:#}"),
            })),
        );
    }
    telemetry::shutdown();
    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let cmd_name = format!("{:?}", cli.command)
        .split(&[' ', '('][..])
        .next()
        .unwrap_or("unknown")
        .to_lowercase();
    {
        let props = serde_json::json!({
            "command": cmd_name,
        });
        telemetry::record_event("command.run", Some(props));
    }
    let result = match &cli.command {
        Commands::Setup => setup::run(),
        Commands::Open { .. } => open::run(match &cli.command {
            Commands::Open { uri } => uri,
            _ => unreachable!(),
        }),
        Commands::Status => status::run(),
        Commands::Restart => restart::run(),
        Commands::Doctor => doctor::run(),
        Commands::Install {
            interactive: true, ..
        } => addon::install_interactive(),
        Commands::Install {
            addon: Some(name), ..
        } => addon::install(name),
        Commands::Install { addon: None, .. } => {
            eprintln!("Usage: tlink install <addon-name>");
            eprintln!("       tlink install --interactive");
            eprintln!("Run `tlink list add-ons` to see available add-ons.");
            Ok(())
        }
        Commands::Delete { addon } => addon::delete(addon),
        Commands::List {
            target: ListTarget::Addons,
        } => addon::list(),
        Commands::Telemetry { action } => match action {
            TelemetryAction::Enable { dsn } => telemetry::enable(dsn.clone()),
            TelemetryAction::Disable => telemetry::disable(),
            TelemetryAction::Status => telemetry::status(),
        },
        Commands::Notify {
            session,
            window,
            pane,
            term,
            source,
        } => notify::run(session, window, pane, term, source),
    };
    if result.is_ok() {
        telemetry::record_event(
            "command.ok",
            Some(serde_json::json!({
                "command": cmd_name,
            })),
        );
    }
    result
}
