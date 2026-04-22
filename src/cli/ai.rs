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
        AiCommand::Setup => rt.block_on(run_setup()),
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
        println!("{:<7} {}", "Score", "Path");
        println!("{:<7} {}", "-----", "----");
        for (path, score) in &results {
            println!("{score:<7.3} {path}");
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

async fn run_setup() -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let ai = &vault.config.ai;
    let ollama_url = ai.ollama_url.as_deref().unwrap_or("http://localhost:11434");

    println!("AI Configuration (.openidiom/config.toml)");
    println!("==========================================\n");

    // Probe Ollama once if either provider needs it
    let needs_ollama = ai.provider == "ollama" || ai.embedding_provider == "ollama";
    let ollama_status = if needs_ollama {
        probe_ollama(ollama_url).await
    } else {
        OllamaStatus::NotNeeded
    };

    // LLM provider
    println!("LLM provider:       {}", ai.provider);
    if let Some(ref model) = ai.model {
        println!("  model:            {model}");
    }
    let provider_ok = check_provider_ready(&ai.provider, &ollama_status);

    // Embedding provider
    println!("\nEmbedding provider: {}", ai.embedding_provider);
    println!("  model:            {}", ai.embedding_model);
    let embedder_ok = check_embedder_ready(
        &ai.embedding_provider,
        &ai.embedding_model,
        ollama_url,
        &ollama_status,
    );

    // Summary
    println!();
    if provider_ok && embedder_ok {
        println!("All good. Your AI configuration is ready to use.");
    } else {
        println!("Issues found:\n");
        if !provider_ok {
            print_provider_fix(&ai.provider, &ollama_status);
        }
        if !embedder_ok {
            print_embedder_fix(&ai.embedding_provider, &ai.embedding_model, &ai.provider, &ollama_status);
        }
        println!("\nConfig file: {}/.openidiom/config.toml", vault.root.display());
    }

    Ok(ExitCode::SUCCESS)
}

// -- Ollama probing --------------------------------------------------------

enum OllamaStatus {
    /// Ollama is not used by any provider.
    NotNeeded,
    /// Ollama responded; contains the list of model names available.
    Running(Vec<String>),
    /// Could not reach Ollama.
    Unreachable(String),
}

/// Ping Ollama's `/api/tags` endpoint and return available models.
async fn probe_ollama(base_url: &str) -> OllamaStatus {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            // Response shape: { "models": [ { "name": "nomic-embed-text:latest", ... }, ... ] }
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                let models = body["models"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m["name"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                OllamaStatus::Running(models)
            } else {
                OllamaStatus::Running(vec![])
            }
        }
        Ok(resp) => OllamaStatus::Unreachable(format!("HTTP {}", resp.status())),
        Err(e) => OllamaStatus::Unreachable(e.to_string()),
    }
}

/// Check if a model (or its `:latest` variant) is in the list.
fn ollama_has_model(models: &[String], wanted: &str) -> bool {
    let wanted_lower = wanted.to_lowercase();
    models.iter().any(|m| {
        let m_lower = m.to_lowercase();
        m_lower == wanted_lower
            || m_lower == format!("{wanted_lower}:latest")
            || m_lower.starts_with(&format!("{wanted_lower}:"))
    })
}

// -- Provider checks -------------------------------------------------------

fn check_provider_ready(provider: &str, ollama: &OllamaStatus) -> bool {
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
            print_ollama_reachability(ollama);
            !matches!(ollama, OllamaStatus::Unreachable(_))
        }
        _ => {
            println!("  unknown provider");
            false
        }
    }
}

fn check_embedder_ready(
    provider: &str,
    model: &str,
    ollama_url: &str,
    ollama: &OllamaStatus,
) -> bool {
    match provider {
        "openai" => {
            let ok = std::env::var("OPENAI_API_KEY").is_ok();
            println!("  OPENAI_API_KEY:   {}", if ok { "set" } else { "MISSING" });
            ok
        }
        "ollama" => {
            println!("  url:              {ollama_url}");
            println!("  (no API key needed)");
            match ollama {
                OllamaStatus::Running(models) => {
                    if ollama_has_model(models, model) {
                        println!("  model status:     installed");
                        true
                    } else {
                        println!("  model status:     NOT FOUND");
                        println!("    Run: ollama pull {model}");
                        false
                    }
                }
                OllamaStatus::Unreachable(_) => {
                    print_ollama_reachability(ollama);
                    false
                }
                OllamaStatus::NotNeeded => true,
            }
        }
        _ => {
            println!("  unknown provider");
            false
        }
    }
}

fn print_ollama_reachability(ollama: &OllamaStatus) {
    match ollama {
        OllamaStatus::Running(_) => println!("  ollama:           reachable"),
        OllamaStatus::Unreachable(reason) => {
            println!("  ollama:           NOT REACHABLE ({reason})");
        }
        OllamaStatus::NotNeeded => {}
    }
}

// -- Fix suggestions -------------------------------------------------------

fn print_provider_fix(provider: &str, ollama: &OllamaStatus) {
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
        "ollama" => {
            if matches!(ollama, OllamaStatus::Unreachable(_)) {
                print_ollama_install_help();
            }
        }
        _ => {}
    }
}

fn print_embedder_fix(
    embedding_provider: &str,
    embedding_model: &str,
    llm_provider: &str,
    ollama: &OllamaStatus,
) {
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
        "ollama" => {
            if matches!(ollama, OllamaStatus::Unreachable(_)) {
                print_ollama_install_help();
            } else if let OllamaStatus::Running(models) = ollama {
                if !ollama_has_model(models, embedding_model) {
                    println!("  Embedding model '{embedding_model}' is not installed in Ollama.");
                    println!("    ollama pull {embedding_model}");
                }
            }
        }
        _ => {}
    }
}

fn print_ollama_install_help() {
    println!();
    println!("  Ollama is not reachable. Install and start it:");
    println!();
    match std::env::consts::OS {
        "linux" => {
            println!("    curl -fsSL https://ollama.com/install.sh | sh");
            println!("    ollama serve        # start in background, or use systemd");
        }
        "macos" => {
            println!("    brew install ollama");
            println!("    ollama serve        # or launch the Ollama app");
        }
        "windows" => {
            println!("    winget install Ollama.Ollama");
            println!("    ollama serve        # run in a terminal to start the server");
        }
        other => {
            println!("    See https://ollama.com/download for {other} instructions.");
        }
    }
    // Detect WSL and add a note about using Windows Ollama
    if std::env::consts::OS == "linux" && is_wsl() {
        println!();
        println!("  WSL detected. If Ollama is installed on Windows instead, it should");
        println!("  be reachable at localhost:11434 automatically. Make sure 'ollama serve'");
        println!("  is running on the Windows side.");
    }
    println!();
    println!("  After installing, pull the default embedding model:");
    println!("    ollama pull nomic-embed-text");
}

fn is_wsl() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|v| v.to_lowercase().contains("microsoft"))
        .unwrap_or(false)
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
