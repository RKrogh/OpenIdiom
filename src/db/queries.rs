use std::path::Path;

use rusqlite::Connection;

use crate::core::parser::ParsedNote;

pub fn get_content_hash(conn: &Connection, path: &Path) -> Result<Option<String>, rusqlite::Error> {
    let path_str = path.to_string_lossy();
    let mut stmt = conn.prepare("SELECT content_hash FROM notes WHERE path = ?1")?;
    let mut rows = stmt.query_map([path_str.as_ref()], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(Ok(hash)) => Ok(Some(hash)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

pub fn upsert_note(
    conn: &Connection,
    note: &ParsedNote,
    content_hash: &str,
) -> Result<(), rusqlite::Error> {
    let path_str = note.path.to_string_lossy().to_string();
    let fm_json = note.frontmatter.as_ref().map(|v| v.to_string());
    let now = chrono::Utc::now().to_rfc3339();

    // Delete existing data for this path (cascade handles links, tags, headings)
    conn.execute("DELETE FROM notes WHERE path = ?1", [&path_str])?;

    conn.execute(
        "INSERT INTO notes (path, title, word_count, frontmatter_json, content_hash, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            path_str,
            note.title,
            note.word_count as i64,
            fm_json,
            content_hash,
            now,
        ],
    )?;

    let note_id = conn.last_insert_rowid();

    // Insert links
    for link in &note.wikilinks {
        conn.execute(
            "INSERT INTO links (source_id, target_title, alias, line)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![note_id, link.target, link.alias, link.line as i64],
        )?;
    }

    // Insert tags
    for tag in &note.tags {
        conn.execute(
            "INSERT INTO tags (note_id, tag) VALUES (?1, ?2)",
            rusqlite::params![note_id, tag],
        )?;
    }

    // Insert headings
    for heading in &note.headings {
        conn.execute(
            "INSERT INTO headings (note_id, text, level, line)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![note_id, heading.text, heading.level as i64, heading.line as i64],
        )?;
    }

    // Update FTS index
    conn.execute(
        "INSERT INTO notes_fts (rowid, title, content) VALUES (?1, ?2, ?3)",
        rusqlite::params![note_id, note.title, note.body],
    )?;

    Ok(())
}

pub fn get_all_unresolved_links(conn: &Connection) -> Result<Vec<(i64, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, target_title FROM links WHERE target_id IS NULL"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect()
}

pub fn get_note_id_by_path(conn: &Connection, path: &Path) -> Result<Option<i64>, rusqlite::Error> {
    let path_str = path.to_string_lossy();
    let mut stmt = conn.prepare("SELECT id FROM notes WHERE path = ?1")?;
    let mut rows = stmt.query_map([path_str.as_ref()], |row| row.get::<_, i64>(0))?;
    match rows.next() {
        Some(Ok(id)) => Ok(Some(id)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

pub fn update_link_target(
    conn: &Connection,
    link_id: i64,
    target_id: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE links SET target_id = ?1 WHERE id = ?2",
        rusqlite::params![target_id, link_id],
    )?;
    Ok(())
}

pub fn count_notes(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
}

pub fn count_links(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
}

pub fn count_unique_tags(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.query_row("SELECT COUNT(DISTINCT tag) FROM tags", [], |row| row.get(0))
}

/// FTS5 full-text search, returns (path, title, snippet) sorted by relevance
#[allow(dead_code)]
pub fn search_fts(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<(String, String, f64)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT n.path, n.title, rank
         FROM notes_fts f
         JOIN notes n ON n.id = f.rowid
         WHERE notes_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2"
    )?;

    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;

    rows.collect()
}
