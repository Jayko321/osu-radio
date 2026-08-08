pub(crate) mod commands;
pub(crate) mod consts;
pub(crate) mod types;

use std::process::ExitCode;

use clap::Parser;

use crate::{
    commands::{import::import, scan::scan},
    consts::PROJECT_HELP,
    types::{Cli, Command},
};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command.unwrap_or(Command::Help) {
        Command::Help => {
            println!("{PROJECT_HELP}");
            Ok(())
        }
        Command::Scan(args) => scan(args).await,
        Command::Import(args) => import(args).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
