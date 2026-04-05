mod init;
mod index;
mod status;
mod query;
mod search;
mod check;
mod graph;
mod daily;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "oi", about = "OpenIdiom — headless knowledge base CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new vault in the current directory
    Init,
    /// Index all Markdown files in the vault
    Index {
        /// Re-index everything, ignoring content hashes
        #[arg(long)]
        force: bool,
        /// Print detailed statistics
        #[arg(long)]
        stats: bool,
    },
    /// Show vault status and health
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Query notes by tags, links, frontmatter
    Query(query::QueryArgs),
    /// Full-text keyword search
    Search(search::SearchArgs),
    /// Run vault health checks
    Check(check::CheckArgs),
    /// Export the link graph
    Graph(graph::GraphArgs),
    /// Create or print daily note path
    Daily(daily::DailyArgs),
    /// AI-powered commands
    Ai,
}

pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Init => init::run(),
        Command::Index { force, stats } => index::run(force, stats),
        Command::Status { json } => status::run(json),
        Command::Query(args) => query::run(args),
        Command::Search(args) => search::run(args),
        Command::Check(args) => check::run(args),
        Command::Graph(args) => graph::run(args),
        Command::Daily(args) => daily::run(args),
        Command::Ai => todo!("Phase 3"),
    }
}
