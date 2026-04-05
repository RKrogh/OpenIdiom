use anyhow::Result;
use rusqlite::Connection;

use crate::ai::providers::AnyEmbedder;
use crate::ai::semantic;
use crate::core::vault::Vault;

/// Assemble RAG context: embed query, search, load chunks from disk
pub async fn assemble_rag_context(
    conn: &Connection,
    vault: &Vault,
    embedder: &AnyEmbedder,
    query: &str,
    top_k: usize,
) -> Result<(String, Vec<String>)> {
    // Embed the query
    let query_vecs = embedder.embed(&[query.to_string()]).await?;
    let query_vec = query_vecs.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("Failed to embed query"))?;

    // Vector search
    let results = semantic::vector_search(conn, &query_vec, top_k)?;

    if results.is_empty() {
        return Ok(("No relevant context found in your notes.".into(), vec![]));
    }

    // Load chunk text from disk via byte offsets
    let mut context_parts = Vec::new();
    let mut sources = Vec::new();

    for (_note_id, path, score) in &results {
        let full_path = vault.root.join(path);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let excerpt: String = content.chars().take(500).collect();
            context_parts.push(format!("--- {path} (relevance: {score:.2}) ---\n{excerpt}\n"));
            if !sources.contains(path) {
                sources.push(path.clone());
            }
        }
    }

    let context = context_parts.join("\n");
    Ok((context, sources))
}

/// Build system prompt for RAG queries
pub fn rag_system_prompt() -> &'static str {
    "You are a helpful assistant answering questions based ONLY on the provided note context. \
     Cite your sources by note filename. If the context doesn't contain enough information \
     to answer the question, say so."
}

/// Build prompt for connection suggestions
pub fn connect_system_prompt() -> &'static str {
    "You are analyzing a knowledge base. Given a note and related notes, suggest meaningful \
     connections and links between them. Explain why each connection is relevant. \
     Format: 'note-a.md could link to note-b.md (reason)'"
}
