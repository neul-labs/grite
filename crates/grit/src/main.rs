mod cli;
mod commands;
mod context;
mod output;
mod event_helper;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Command::Init => commands::init::run(&cli),
        Command::Actor { cmd } => commands::actor::run(&cli, cmd.clone()),
        Command::Issue { cmd } => commands::issue::run(&cli, cmd.clone()),
        Command::Db { cmd } => commands::db::run(&cli, cmd.clone()),
        Command::Export { format, since } => commands::export::run(&cli, format.clone(), since.clone()),
        Command::Rebuild => commands::rebuild::run(&cli),
        Command::Sync { remote, pull, push } => commands::sync::run(&cli, remote.clone(), *pull, *push),
        Command::Snapshot { cmd } => commands::snapshot::run(&cli, cmd.clone()),
    };

    if let Err(e) = result {
        output::output_error(&cli, &e);
        std::process::exit(e.exit_code());
    }
}
