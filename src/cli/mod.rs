mod init;
mod index;
mod status;

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
    Query,
    /// Full-text keyword search
    Search,
    /// Run vault health checks
    Check,
    /// Export the link graph
    Graph,
    /// Create or print daily note path
    Daily,
    /// AI-powered commands
    Ai,
}

pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Init => init::run(),
        Command::Index { force, stats } => index::run(force, stats),
        Command::Status { json } => status::run(json),
        Command::Query => todo!("Phase 2"),
        Command::Search => todo!("Phase 2"),
        Command::Check => todo!("Phase 2"),
        Command::Graph => todo!("Phase 2"),
        Command::Daily => todo!("Phase 2"),
        Command::Ai => todo!("Phase 3"),
    }
}
