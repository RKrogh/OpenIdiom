use std::fmt;
use std::path::Path;

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::core::link_resolver::LinkResolver;
use crate::core::parser;
use crate::core::vault::Vault;
use crate::db;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Parser error: {0}")]
    Parser(#[from] parser::ParserError),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Default)]
pub struct IndexStats {
    pub total_notes: usize,
    pub total_links: usize,
    pub total_tags: usize,
    pub new_notes: usize,
    pub updated_notes: usize,
    pub skipped_notes: usize,
    pub ambiguous_links: usize,
    pub broken_links: usize,
}

impl fmt::Display for IndexStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Notes: {} total ({} new, {} updated, {} skipped)",
            self.total_notes, self.new_notes, self.updated_notes, self.skipped_notes)?;
        writeln!(f, "Links: {} total ({} broken, {} ambiguous)",
            self.total_links, self.broken_links, self.ambiguous_links)?;
        writeln!(f, "Tags: {} unique", self.total_tags)?;
        Ok(())
    }
}

pub fn index_vault(
    conn: &Connection,
    vault: &Vault,
    force: bool,
) -> Result<IndexStats, IndexError> {
    let mut stats = IndexStats::default();
    let mut resolver = LinkResolver::new();

    let ignore = &vault.config.vault.ignore;

    // First pass: parse all notes, insert into DB, register with resolver
    for entry in WalkDir::new(&vault.root)
        .into_iter()
        .filter_entry(|e| !should_ignore(e.path(), &vault.root, ignore))
    {
        let entry = entry.map_err(std::io::Error::other)?;
        let path = entry.path();

        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = std::fs::read_to_string(path)?;
        let hash = compute_hash(&content);
        let rel_path = path.strip_prefix(&vault.root).unwrap_or(path);

        // Check if already indexed with same hash
        if !force {
            if let Some(existing_hash) = db::queries::get_content_hash(conn, rel_path)? {
                if existing_hash == hash {
                    stats.skipped_notes += 1;
                    // Still register for link resolution
                    let title = rel_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("untitled");
                    resolver.register(title, rel_path);
                    continue;
                }
                stats.updated_notes += 1;
            } else {
                stats.new_notes += 1;
            }
        } else {
            stats.new_notes += 1;
        }

        let parsed = parser::parse_note(&content, rel_path)?;
        // Register both the frontmatter title and filename stem for resolution.
        // Wikilinks typically use the filename: [[api-design]], not [[API Design Patterns]].
        resolver.register(&parsed.title, rel_path);
        let stem = rel_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        if stem.to_lowercase() != parsed.title.to_lowercase() {
            resolver.register(stem, rel_path);
        }
        db::queries::upsert_note(conn, &parsed, &hash)?;
    }

    // Second pass: resolve links
    let all_links = db::queries::get_all_unresolved_links(conn)?;
    for (link_id, target_title) in &all_links {
        let resolved = resolver.resolve(target_title);
        if let Some(ref path) = resolved.resolved_path
            && let Some(note_id) = db::queries::get_note_id_by_path(conn, path)?
        {
            db::queries::update_link_target(conn, *link_id, note_id)?;
        }
        if resolved.ambiguous {
            stats.ambiguous_links += 1;
        }
        if resolved.resolved_path.is_none() {
            stats.broken_links += 1;
        }
    }

    // Gather final stats
    stats.total_notes = db::queries::count_notes(conn)?;
    stats.total_links = db::queries::count_links(conn)?;
    stats.total_tags = db::queries::count_unique_tags(conn)?;

    Ok(stats)
}

fn should_ignore(path: &Path, root: &Path, ignore: &[String]) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    for component in rel.components() {
        let name = component.as_os_str().to_string_lossy();
        for pattern in ignore {
            if name == *pattern {
                return true;
            }
        }
    }
    false
}

fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
