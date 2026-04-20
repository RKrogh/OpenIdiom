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
    /// Check AI configuration and diagnose issues
    Setup,
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
        AiCommand::Setup => run_setup(),
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

fn run_setup() -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let ai = &vault.config.ai;

    println!("AI Configuration (.openidiom/config.toml)");
    println!("==========================================\n");

    // LLM provider
    println!("LLM provider:       {}", ai.provider);
    if let Some(ref model) = ai.model {
        println!("  model:            {model}");
    }
    let provider_ok = check_provider_ready(&ai.provider);

    // Embedding provider
    println!("\nEmbedding provider: {}", ai.embedding_provider);
    println!("  model:            {}", ai.embedding_model);
    let embedder_ok = check_embedder_ready(&ai.embedding_provider, ai.ollama_url.as_deref());

    // Summary
    println!();
    if provider_ok && embedder_ok {
        println!("All good. Your AI configuration is ready to use.");
    } else {
        println!("Issues found:\n");
        if !provider_ok {
            print_provider_fix(&ai.provider);
        }
        if !embedder_ok {
            print_embedder_fix(&ai.embedding_provider, &ai.provider);
        }
        println!("\nConfig file: {}/.openidiom/config.toml", vault.root.display());
    }

    Ok(ExitCode::SUCCESS)
}

/// Check if the LLM provider is ready, printing status.
fn check_provider_ready(provider: &str) -> bool {
    match provider {
        "claude" => {
            let ok = std::env::var("ANTHROPIC_API_KEY").is_ok();
            println!("  ANTHROPIC_API_KEY: {}", if ok { "set" } else { "MISSING" });
            ok
        }
        "openai" => {
            let ok = std::env::var("OPENAI_API_KEY").is_ok();
            println!("  OPENAI_API_KEY:   {}", if ok { "set" } else { "MISSING" });
            ok
        }
        "ollama" => {
            println!("  (no API key needed)");
            true
        }
        _ => {
            println!("  unknown provider");
            false
        }
    }
}

/// Check if the embedding provider is ready, printing status.
fn check_embedder_ready(provider: &str, ollama_url: Option<&str>) -> bool {
    match provider {
        "openai" => {
            let ok = std::env::var("OPENAI_API_KEY").is_ok();
            println!("  OPENAI_API_KEY:   {}", if ok { "set" } else { "MISSING" });
            ok
        }
        "ollama" => {
            let url = ollama_url.unwrap_or("http://localhost:11434");
            println!("  url:              {url}");
            println!("  (no API key needed)");
            true
        }
        _ => {
            println!("  unknown provider");
            false
        }
    }
}

fn print_provider_fix(provider: &str) {
    match provider {
        "claude" => {
            println!("  LLM: ANTHROPIC_API_KEY is not set.");
            println!("    export ANTHROPIC_API_KEY=sk-ant-...");
            println!("    Or switch to ollama (free, local): set provider = \"ollama\" in config");
        }
        "openai" => {
            println!("  LLM: OPENAI_API_KEY is not set.");
            println!("    export OPENAI_API_KEY=sk-...");
            println!("    Or switch to ollama (free, local): set provider = \"ollama\" in config");
        }
        _ => {}
    }
}

fn print_embedder_fix(embedding_provider: &str, llm_provider: &str) {
    match embedding_provider {
        "openai" => {
            println!("  Embeddings: OPENAI_API_KEY is not set.");
            if llm_provider != "openai" {
                println!("    Note: embeddings need a separate provider from the LLM.");
                println!("    Even with provider = \"{llm_provider}\", embeddings still need their own key.");
            }
            println!();
            println!("    Option 1: export OPENAI_API_KEY=sk-...");
            println!("    Option 2: switch to Ollama for free local embeddings:");
            println!("      In .openidiom/config.toml, set:");
            println!("        embedding_provider = \"ollama\"");
            println!("        embedding_model = \"nomic-embed-text\"");
            println!("      Then run: ollama pull nomic-embed-text");
        }
        _ => {}
    }
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
