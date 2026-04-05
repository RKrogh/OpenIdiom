#![warn(clippy::all)]

mod cli;
mod core;
mod db;
mod output;

use clap::Parser;
use cli::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli::run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(3)
        }
    }
}
