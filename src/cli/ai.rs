use std::process::ExitCode;

use clap::Subcommand;

#[derive(clap::Args)]
pub struct AiArgs {
    #[command(subcommand)]
    pub command: AiCommand,
}

#[derive(Subcommand)]
pub enum AiCommand {
    /// Embed all notes for semantic search
    Index {
        /// Re-embed everything
        #[arg(long)]
        force: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
        /// Show cost estimate only, don't embed
        #[arg(long)]
        dry_run: bool,
    },
    /// Semantic search over note embeddings
    Search {
        /// Search query
        query: String,
    },
    /// Ask a question (RAG over your notes)
    Ask {
        /// The question to ask
        question: String,
        /// Disable streaming (return complete response)
        #[arg(long)]
        no_stream: bool,
    },
    /// Suggest connections for a note
    Connect {
        /// Note name or title
        note: String,
        /// Disable streaming
        #[arg(long)]
        no_stream: bool,
    },
    /// Summarize notes matching a filter
    Summarize {
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Disable streaming
        #[arg(long)]
        no_stream: bool,
    },
    /// Show AI usage metrics
    Metrics,
}

pub fn run(args: AiArgs) -> anyhow::Result<ExitCode> {
    // Build a tokio runtime for async AI calls
    let rt = tokio::runtime::Runtime::new()?;

    match args.command {
        AiCommand::Index { force, yes, dry_run } => rt.block_on(run_index(force, yes, dry_run)),
        AiCommand::Search { query } => rt.block_on(run_search(&query)),
        AiCommand::Ask { question, no_stream: _ } => rt.block_on(run_ask(&question)),
        AiCommand::Connect { note, no_stream: _ } => rt.block_on(run_connect(&note)),
        AiCommand::Summarize { tag, no_stream: _ } => rt.block_on(run_summarize(tag.as_deref())),
        AiCommand::Metrics => run_metrics(),
    }
}

async fn run_index(force: bool, yes: bool, dry_run: bool) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;
    ensure_metrics_table(&conn)?;

    let embedder = crate::ai::providers::create_embedder(&vault.config.ai)?;

    if dry_run {
        crate::ai::cost::print_cost_estimate(&conn, &vault, &embedder)?;
        return Ok(ExitCode::SUCCESS);
    }

    if !yes {
        crate::ai::cost::print_cost_estimate(&conn, &vault, &embedder)?;
        // In non-interactive (test) mode, just proceed
    }

    let stats = crate::ai::semantic::embed_vault(&conn, &vault, &embedder, force).await?;
    println!("{stats}");

    Ok(ExitCode::SUCCESS)
}

async fn run_search(query: &str) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;

    // Check if embeddings exist
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;
    if count == 0 {
        println!("No embeddings found. Run `oi ai index` first.");
        return Ok(ExitCode::SUCCESS);
    }

    let embedder = crate::ai::providers::create_embedder(&vault.config.ai)?;
    let query_vecs = embedder.embed(&[query.to_string()]).await?;
    let query_vec = query_vecs.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("Failed to embed query"))?;

    let results = crate::ai::commands::ai_search(&conn, &query_vec, vault.config.ai.search_top_k)?;

    if results.is_empty() {
        println!("No results");
    } else {
        for (path, score) in &results {
            println!("{score:.3}  {path}");
        }
    }

    Ok(ExitCode::SUCCESS)
}

async fn run_ask(question: &str) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;
    ensure_metrics_table(&conn)?;

    let provider = crate::ai::providers::create_provider(&vault.config.ai)?;
    let embedder = crate::ai::providers::create_embedder(&vault.config.ai)?;

    let (answer, sources) = crate::ai::commands::ai_ask(&conn, &vault, &provider, &embedder, question).await?;

    println!("{answer}");
    if !sources.is_empty() {
        println!("\nSources: {}", sources.join(", "));
    }

    Ok(ExitCode::SUCCESS)
}

async fn run_connect(note: &str) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;
    ensure_metrics_table(&conn)?;

    let provider = crate::ai::providers::create_provider(&vault.config.ai)?;
    let embedder = crate::ai::providers::create_embedder(&vault.config.ai)?;

    let suggestion = crate::ai::commands::ai_connect(&conn, &vault, &provider, &embedder, note).await?;
    println!("{suggestion}");

    Ok(ExitCode::SUCCESS)
}

async fn run_summarize(tag: Option<&str>) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;
    ensure_metrics_table(&conn)?;

    let provider = crate::ai::providers::create_provider(&vault.config.ai)?;

    let summary = crate::ai::commands::ai_summarize(&conn, &vault, &provider, tag).await?;
    println!("{summary}");

    Ok(ExitCode::SUCCESS)
}

fn run_metrics() -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;

    crate::ai::cost::print_metrics(&conn)?;

    Ok(ExitCode::SUCCESS)
}

fn ensure_metrics_table(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metrics (
            id INTEGER PRIMARY KEY,
            operation TEXT,
            key TEXT,
            value INTEGER,
            timestamp TEXT
        )"
    )?;
    Ok(())
}
