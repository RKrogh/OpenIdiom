use anyhow::Result;
use rusqlite::Connection;

use crate::ai::context;
use crate::ai::providers::{AnyProvider, AnyEmbedder};
use crate::ai::semantic;
use crate::core::vault::Vault;

/// Semantic search over embeddings (no LLM call — just vector similarity)
pub fn ai_search(
    conn: &Connection,
    query_vec: &[f32],
    top_k: usize,
) -> Result<Vec<(String, f64)>> {
    let results = semantic::vector_search(conn, query_vec, top_k)?;
    Ok(results.into_iter().map(|(_, path, score)| (path, score)).collect())
}

/// RAG-powered question answering
pub async fn ai_ask(
    conn: &Connection,
    vault: &Vault,
    provider: &AnyProvider,
    embedder: &AnyEmbedder,
    question: &str,
) -> Result<(String, Vec<String>)> {
    let top_k = vault.config.ai.context_top_k;
    let (rag_context, sources) = context::assemble_rag_context(conn, vault, embedder, question, top_k).await?;

    let prompt = format!(
        "Context from my notes:\n\n{rag_context}\n\nQuestion: {question}"
    );

    let answer = provider.complete(&prompt, Some(context::rag_system_prompt())).await?;

    // Store metrics
    store_completion_metric(conn, question.len(), answer.len());

    Ok((answer, sources))
}

/// Suggest connections for a note
pub async fn ai_connect(
    conn: &Connection,
    vault: &Vault,
    provider: &AnyProvider,
    embedder: &AnyEmbedder,
    note_name: &str,
) -> Result<String> {
    // Find the note
    let note_path: String = conn.query_row(
        "SELECT path FROM notes WHERE title LIKE ?1 OR path LIKE ?2",
        rusqlite::params![
            format!("%{note_name}%"),
            format!("%{note_name}%"),
        ],
        |row| row.get(0),
    )?;

    let content = std::fs::read_to_string(vault.root.join(&note_path))?;

    // Get semantically similar notes
    let top_k = vault.config.ai.context_top_k;
    let (rag_context, _sources) = context::assemble_rag_context(conn, vault, embedder, &content, top_k).await?;

    let prompt = format!(
        "Here is the note '{note_path}':\n\n{content}\n\n\
         Here are potentially related notes:\n\n{rag_context}\n\n\
         Suggest meaningful connections between the main note and the related notes."
    );

    provider.complete(&prompt, Some(context::connect_system_prompt())).await
}

/// Summarize notes matching a filter
pub async fn ai_summarize(
    conn: &Connection,
    vault: &Vault,
    provider: &AnyProvider,
    tag: Option<&str>,
) -> Result<String> {
    let notes: Vec<(String, String)> = if let Some(tag) = tag {
        let mut stmt = conn.prepare(
            "SELECT n.path, n.title FROM notes n
             JOIN tags t ON t.note_id = n.id
             WHERE t.tag = ?1"
        )?;
        stmt.query_map([tag], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?
    } else {
        let mut stmt = conn.prepare("SELECT path, title FROM notes")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?
    };

    let mut content_parts = Vec::new();
    for (path, title) in &notes {
        let full_path = vault.root.join(path);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let excerpt: String = content.chars().take(1000).collect();
            content_parts.push(format!("--- {title} ({path}) ---\n{excerpt}"));
        }
    }

    let combined = content_parts.join("\n\n");
    let prompt = format!(
        "Summarize the following {} notes:\n\n{combined}",
        notes.len()
    );

    provider.complete(&prompt, Some("Provide a concise summary of the notes, highlighting key themes and connections.")).await
}

fn store_completion_metric(conn: &Connection, prompt_len: usize, response_len: usize) {
    let _ = conn.execute(
        "INSERT INTO metrics (operation, key, value, timestamp)
         VALUES ('completion', 'tokens_sent', ?1, datetime('now'))",
        [prompt_len as i64 / 4],
    );
    let _ = conn.execute(
        "INSERT INTO metrics (operation, key, value, timestamp)
         VALUES ('completion', 'tokens_received', ?1, datetime('now'))",
        [response_len as i64 / 4],
    );
}
