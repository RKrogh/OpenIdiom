use rusqlite::Connection;

pub fn create_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;

        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            title TEXT NOT NULL,
            word_count INTEGER,
            frontmatter_json TEXT,
            content_hash TEXT,
            indexed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS links (
            id INTEGER PRIMARY KEY,
            source_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
            target_title TEXT NOT NULL,
            target_id INTEGER REFERENCES notes(id) ON DELETE SET NULL,
            alias TEXT,
            line INTEGER
        );

        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            note_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
            tag TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS headings (
            id INTEGER PRIMARY KEY,
            note_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
            text TEXT NOT NULL,
            level INTEGER NOT NULL,
            line INTEGER
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            title,
            content,
            content_rowid='id',
            tokenize='porter unicode61'
        );

        CREATE TABLE IF NOT EXISTS embeddings (
            id INTEGER PRIMARY KEY,
            note_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
            chunk_index INTEGER,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            preview TEXT,
            embedding BLOB,
            model TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);
        CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_id);
        CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_id);
        CREATE INDEX IF NOT EXISTS idx_notes_title ON notes(title);
        CREATE INDEX IF NOT EXISTS idx_embeddings_note ON embeddings(note_id);
        "
    )?;

    Ok(())
}
