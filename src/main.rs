mod cli;
mod config;
mod open;
mod bundle;
mod terminal;
mod status;
mod restart;
mod setup;
mod doctor;
mod addon;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ListTarget};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Setup => setup::run(),
        Commands::Open { uri } => open::run(&uri),
        Commands::Status => status::run(),
        Commands::Restart => restart::run(),
        Commands::Doctor => doctor::run(),
        Commands::Install { addon } => addon::install(&addon),
        Commands::Delete { addon } => addon::delete(&addon),
        Commands::List { target: ListTarget::Addons } => addon::list(),
    }
}
