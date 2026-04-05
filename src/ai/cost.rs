use anyhow::Result;
use rusqlite::Connection;

use crate::ai::providers::AnyEmbedder;
use crate::core::vault::Vault;

pub fn print_cost_estimate(
    conn: &Connection,
    vault: &Vault,
    embedder: &AnyEmbedder,
) -> Result<()> {
    let (notes, tokens, cost) = super::semantic::estimate_embedding_cost(conn, vault, embedder)?;

    println!("Embedding estimate:");
    println!("  Notes: {notes}");
    println!("  Estimated tokens: ~{tokens}");

    if let Some(c) = cost {
        println!("  Estimated cost: ~${:.4}", c);
    } else {
        println!("  Cost: free (local provider)");
    }

    Ok(())
}

pub fn print_metrics(conn: &Connection) -> Result<()> {
    // Ensure table exists
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metrics (
            id INTEGER PRIMARY KEY,
            operation TEXT,
            key TEXT,
            value INTEGER,
            timestamp TEXT
        )"
    )?;

    let mut stmt = conn.prepare(
        "SELECT operation, key, SUM(value) FROM metrics GROUP BY operation, key ORDER BY operation"
    )?;

    let mut rows = stmt.query([])?;
    let mut has_data = false;

    println!("AI Usage Metrics:");
    while let Some(row) = rows.next()? {
        let op: String = row.get(0)?;
        let key: String = row.get(1)?;
        let total: i64 = row.get(2)?;
        println!("  [{op}] {key}: {total}");
        has_data = true;
    }

    if !has_data {
        println!("  No usage recorded yet.");
    }

    Ok(())
}
