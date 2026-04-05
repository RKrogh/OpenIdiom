use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone)]
#[allow(dead_code)] // And variant reserved for future DSL parser
pub enum Filter {
    Tag(String),
    Link(String),
    Backlink(String),
    Title(String),
    Frontmatter(String, String),
    MinWords(usize),
    Orphan,
    And(Vec<Filter>),
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub path: String,
    pub title: String,
    pub word_count: i64,
    pub tags: Vec<String>,
}

pub fn execute_query(
    conn: &Connection,
    filters: &[Filter],
) -> Result<Vec<QueryResult>, rusqlite::Error> {
    if filters.is_empty() {
        return list_all_notes(conn);
    }

    // Start with all note IDs, intersect for each filter (AND logic)
    let mut result_ids: Option<Vec<i64>> = None;

    for filter in filters {
        let ids = match filter {
            Filter::Tag(tag) => query_by_tag(conn, tag)?,
            Filter::Link(target) => query_by_link(conn, target)?,
            Filter::Backlink(source) => query_by_backlink(conn, source)?,
            Filter::Title(title) => query_by_title(conn, title)?,
            Filter::Frontmatter(key, value) => query_by_frontmatter(conn, key, value)?,
            Filter::MinWords(min) => query_by_min_words(conn, *min)?,
            Filter::Orphan => query_orphans(conn)?,
            Filter::And(sub) => execute_query_ids(conn, sub)?,
        };

        result_ids = Some(match result_ids {
            None => ids,
            Some(existing) => existing
                .into_iter()
                .filter(|id| ids.contains(id))
                .collect(),
        });
    }

    let ids = result_ids.unwrap_or_default();
    fetch_notes_by_ids(conn, &ids)
}

fn execute_query_ids(
    conn: &Connection,
    filters: &[Filter],
) -> Result<Vec<i64>, rusqlite::Error> {
    let results = execute_query(conn, filters)?;
    Ok(results.iter().map(|r| {
        conn.query_row(
            "SELECT id FROM notes WHERE path = ?1",
            [&r.path],
            |row| row.get(0),
        ).unwrap_or(0)
    }).collect())
}

fn list_all_notes(conn: &Connection) -> Result<Vec<QueryResult>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT n.path, n.title, n.word_count FROM notes n ORDER BY n.title"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (path, title, word_count) = row?;
        let tags = get_tags_for_path(conn, &path)?;
        results.push(QueryResult { path, title, word_count, tags });
    }
    Ok(results)
}

fn query_by_tag(conn: &Connection, tag: &str) -> Result<Vec<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT n.id FROM notes n
         JOIN tags t ON t.note_id = n.id
         WHERE t.tag = ?1"
    )?;
    stmt.query_map([tag], |row| row.get(0))?.collect()
}

fn query_by_link(conn: &Connection, target: &str) -> Result<Vec<i64>, rusqlite::Error> {
    // Notes that link TO the given target (by title or filename)
    let mut stmt = conn.prepare(
        "SELECT DISTINCT l.source_id FROM links l
         WHERE LOWER(l.target_title) = LOWER(?1)"
    )?;
    stmt.query_map([target], |row| row.get(0))?.collect()
}

fn query_by_backlink(conn: &Connection, source: &str) -> Result<Vec<i64>, rusqlite::Error> {
    // Notes that the given source links TO (i.e., targets of source's links)
    let mut stmt = conn.prepare(
        "SELECT DISTINCT l.target_id FROM links l
         JOIN notes n ON n.id = l.source_id
         WHERE (LOWER(n.title) = LOWER(?1)
                OR LOWER(REPLACE(n.path, '.md', '')) LIKE '%' || LOWER(?1))
         AND l.target_id IS NOT NULL"
    )?;
    stmt.query_map([source], |row| row.get(0))?.collect()
}

fn query_by_title(conn: &Connection, title: &str) -> Result<Vec<i64>, rusqlite::Error> {
    let pattern = format!("%{title}%");
    let mut stmt = conn.prepare(
        "SELECT id FROM notes WHERE LOWER(title) LIKE LOWER(?1)"
    )?;
    stmt.query_map([&pattern], |row| row.get(0))?.collect()
}

fn query_by_frontmatter(
    conn: &Connection,
    key: &str,
    value: &str,
) -> Result<Vec<i64>, rusqlite::Error> {
    // Query JSON frontmatter using SQLite JSON functions
    let mut stmt = conn.prepare(
        "SELECT id FROM notes
         WHERE json_extract(frontmatter_json, '$.' || ?1) = ?2"
    )?;
    stmt.query_map(rusqlite::params![key, value], |row| row.get(0))?.collect()
}

fn query_by_min_words(conn: &Connection, min: usize) -> Result<Vec<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id FROM notes WHERE word_count >= ?1"
    )?;
    stmt.query_map([min as i64], |row| row.get(0))?.collect()
}

fn query_orphans(conn: &Connection) -> Result<Vec<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT n.id FROM notes n
         WHERE n.id NOT IN (SELECT DISTINCT source_id FROM links WHERE source_id IS NOT NULL)
         AND n.id NOT IN (SELECT DISTINCT target_id FROM links WHERE target_id IS NOT NULL)"
    )?;
    stmt.query_map([], |row| row.get(0))?.collect()
}

fn fetch_notes_by_ids(
    conn: &Connection,
    ids: &[i64],
) -> Result<Vec<QueryResult>, rusqlite::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let sql = format!(
        "SELECT path, title, word_count FROM notes WHERE id IN ({}) ORDER BY title",
        placeholders.join(",")
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<Box<dyn rusqlite::types::ToSql>> =
        ids.iter().map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>).collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (path, title, word_count) = row?;
        let tags = get_tags_for_path(conn, &path)?;
        results.push(QueryResult { path, title, word_count, tags });
    }
    Ok(results)
}

fn get_tags_for_path(conn: &Connection, path: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT t.tag FROM tags t JOIN notes n ON n.id = t.note_id WHERE n.path = ?1 ORDER BY t.tag"
    )?;
    stmt.query_map([path], |row| row.get(0))?.collect()
}
