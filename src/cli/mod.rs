mod init;
mod index;
mod status;
mod query;
mod search;
mod check;
mod graph;
mod daily;
mod ai;
mod completions;
pub mod mcp;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "oi", about = "OpenIdiom — headless knowledge base CLI")]
pub struct Cli {
    /// Path to vault root (overrides auto-discovery from CWD)
    #[arg(long, global = true)]
    pub vault: Option<PathBuf>,

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
    Ai(ai::AiArgs),
    /// Generate shell completions
    Completions(completions::CompletionsArgs),
    /// Start MCP server
    Mcp(mcp::McpArgs),
}

pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let vault_path = cli.vault.as_deref();
    match cli.command {
        Command::Init => init::run(vault_path),
        Command::Index { force, stats } => index::run(vault_path, force, stats),
        Command::Status { json } => status::run(vault_path, json),
        Command::Query(args) => query::run(vault_path, args),
        Command::Search(args) => search::run(vault_path, args),
        Command::Check(args) => check::run(vault_path, args),
        Command::Graph(args) => graph::run(vault_path, args),
        Command::Daily(args) => daily::run(vault_path, args),
        Command::Ai(args) => ai::run(vault_path, args),
        Command::Completions(args) => completions::run(args),
        Command::Mcp(args) => mcp::run(vault_path, args),
    }
}
