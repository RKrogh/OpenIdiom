use anyhow::Result;
use rusqlite::Connection;

use crate::ai::providers::AnyEmbedder;
use crate::core::vault::Vault;

pub struct EmbedStats {
    pub total_notes: usize,
    pub embedded_notes: usize,
    pub skipped_notes: usize,
    pub total_chunks: usize,
}

impl std::fmt::Display for EmbedStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Embedded {} notes ({} chunks), skipped {} (up to date)",
            self.embedded_notes, self.total_chunks, self.skipped_notes
        )
    }
}

/// Estimate token count and cost for embedding all notes
pub fn estimate_embedding_cost(
    conn: &Connection,
    vault: &Vault,
    embedder: &AnyEmbedder,
) -> Result<(usize, usize, Option<f64>)> {
    let mut total_tokens = 0;
    let mut total_notes = 0;

    let mut stmt = conn.prepare("SELECT path FROM notes")?;
    let paths: Vec<String> = stmt.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?;

    for rel_path in &paths {
        let full_path = vault.root.join(rel_path);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            // Rough token estimate: chars / 4 for English
            total_tokens += content.len() / 4;
            total_notes += 1;
        }
    }

    let cost = embedder.cost_per_token().map(|cpt| cpt * total_tokens as f64);
    Ok((total_notes, total_tokens, cost))
}

/// Embed all notes and store vectors in SQLite
pub async fn embed_vault(
    conn: &Connection,
    vault: &Vault,
    embedder: &AnyEmbedder,
    force: bool,
) -> Result<EmbedStats> {
    let mut stats = EmbedStats {
        total_notes: 0,
        embedded_notes: 0,
        skipped_notes: 0,
        total_chunks: 0,
    };

    let mut stmt = conn.prepare("SELECT id, path, content_hash FROM notes")?;
    let notes: Vec<(i64, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;

    stats.total_notes = notes.len();

    let mut batch_texts = Vec::new();
    let mut batch_meta: Vec<(i64, usize, usize, usize)> = Vec::new(); // (note_id, chunk_idx, start, end)

    for (note_id, rel_path, _hash) in &notes {
        // Check if already embedded with same hash
        if !force {
            let existing: Option<String> = conn
                .query_row(
                    "SELECT model FROM embeddings WHERE note_id = ?1 LIMIT 1",
                    [note_id],
                    |row| row.get(0),
                )
                .ok();
            if existing.is_some() {
                stats.skipped_notes += 1;
                continue;
            }
        }

        let full_path = vault.root.join(rel_path);
        let content = std::fs::read_to_string(&full_path)?;

        // Simple chunking: split on double newlines, combine to ~chunk_size tokens
        let chunks = chunk_content(&content, vault.config.ai.chunk_size);

        // Delete old embeddings for this note
        conn.execute("DELETE FROM embeddings WHERE note_id = ?1", [note_id])?;

        for (i, (start, end, text)) in chunks.iter().enumerate() {
            batch_texts.push(text.clone());
            batch_meta.push((*note_id, i, *start, *end));
        }

        stats.embedded_notes += 1;
    }

    // Batch embed
    if !batch_texts.is_empty() {
        let batch_size = vault.config.ai.batch_size;
        for chunk_start in (0..batch_texts.len()).step_by(batch_size) {
            let chunk_end = (chunk_start + batch_size).min(batch_texts.len());
            let batch = &batch_texts[chunk_start..chunk_end];

            let vectors = embedder.embed(batch).await?;

            let count = vectors.len().min(batch.len());
            for (i, vector) in vectors.iter().enumerate().take(count) {
                let idx = chunk_start + i;
                let (note_id, chunk_idx, start_byte, end_byte) = batch_meta[idx];
                let preview: String = batch_texts[idx].chars().take(200).collect();
                let blob = vector_to_blob(vector);

                conn.execute(
                    "INSERT INTO embeddings (note_id, chunk_index, start_byte, end_byte, preview, embedding, model)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        note_id,
                        chunk_idx as i64,
                        start_byte as i64,
                        end_byte as i64,
                        preview,
                        blob,
                        embedder.model_name(),
                    ],
                )?;

                stats.total_chunks += 1;
            }
        }
    }

    // Store usage metrics
    let total_tokens = batch_texts.iter().map(|t| t.len() / 4).sum::<usize>();
    store_metric(conn, "embed", "tokens_sent", total_tokens as i64)?;
    store_metric(conn, "embed", "notes_embedded", stats.embedded_notes as i64)?;

    Ok(stats)
}

/// Cosine similarity vector search
pub fn vector_search(
    conn: &Connection,
    query_vec: &[f32],
    top_k: usize,
) -> Result<Vec<(i64, String, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT e.note_id, e.preview, e.embedding, n.path
         FROM embeddings e
         JOIN notes n ON n.id = e.note_id"
    )?;

    let mut results: Vec<(i64, String, f64)> = Vec::new();
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let note_id: i64 = row.get(0)?;
        let _preview: String = row.get(1)?;
        let blob: Vec<u8> = row.get(2)?;
        let path: String = row.get(3)?;

        let stored_vec = blob_to_vector(&blob);
        let similarity = cosine_similarity(query_vec, &stored_vec);

        results.push((note_id, path, similarity));
    }

    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    Ok(results)
}

fn chunk_content(content: &str, target_tokens: usize) -> Vec<(usize, usize, String)> {
    let target_chars = target_tokens * 4; // rough token-to-char ratio
    let mut chunks = Vec::new();
    let mut start = 0;

    let paragraphs: Vec<&str> = content.split("\n\n").collect();
    let mut current = String::new();
    let mut current_start = 0;

    for para in paragraphs {
        if current.len() + para.len() > target_chars && !current.is_empty() {
            let end = start + current.len();
            chunks.push((current_start, end, current.clone()));
            current_start = end;
            current.clear();
            start = current_start;
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);
    }

    if !current.is_empty() {
        chunks.push((current_start, current_start + current.len(), current));
    }

    if chunks.is_empty() && !content.is_empty() {
        chunks.push((0, content.len(), content.to_string()));
    }

    chunks
}

fn vector_to_blob(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_vector(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

fn store_metric(conn: &Connection, operation: &str, key: &str, value: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO metrics (operation, key, value, timestamp)
         VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![operation, key, value],
    ).ok(); // Ignore errors — metrics are best-effort
    Ok(())
}
