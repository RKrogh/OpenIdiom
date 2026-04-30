use std::io::{BufRead, Write};
use std::process::ExitCode;

use clap::Subcommand;
use serde_json::json;

#[derive(clap::Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Subcommand)]
pub enum McpCommand {
    /// Start MCP server on stdio
    Serve,
}

pub fn run(vault_path: Option<&std::path::Path>, args: McpArgs) -> anyhow::Result<ExitCode> {
    match args.command {
        McpCommand::Serve => run_server(vault_path),
    }
}

fn run_server(vault_path: Option<&std::path::Path>) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::resolve(vault_path)?;
    let conn = vault.open_db()?;

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = request.get("id").cloned();

        // Notifications (no id) don't get responses
        if id.is_none() {
            continue;
        }

        let response = match method {
            "initialize" => handle_initialize(&request),
            "tools/list" => handle_tools_list(),
            "tools/call" => handle_tool_call(&request, &conn, &vault),
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            }),
        };

        // Merge the id into the response
        let mut resp = response;
        if let Some(id_val) = id {
            resp["id"] = id_val;
        }

        let resp_str = serde_json::to_string(&resp)?;
        writeln!(stdout, "{resp_str}")?;
        stdout.flush()?;
    }

    Ok(ExitCode::SUCCESS)
}

fn handle_initialize(_request: &serde_json::Value) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "openidiom",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn handle_tools_list() -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "tools": [
                {
                    "name": "vault_status",
                    "description": "Get vault name, note count, link count, tag count, and index freshness",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "query_notes",
                    "description": "Query notes by tag, link, title, frontmatter, or orphan status",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tag": { "type": "string", "description": "Filter by tag" },
                            "title": { "type": "string", "description": "Title contains" },
                            "link": { "type": "string", "description": "Notes linking to this target" },
                            "orphan": { "type": "boolean", "description": "Only orphan notes" }
                        }
                    }
                },
                {
                    "name": "search_notes",
                    "description": "Full-text keyword search over note content. Returns path, title, snippet (with >>>match<<< markers), and rank.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query" },
                            "limit": { "type": "integer", "description": "Max results", "default": 10 }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "get_note",
                    "description": "Read a specific note's content and metadata",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Relative path to the note" }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "check_vault",
                    "description": "Run vault health checks (broken links, orphans, dead tags)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "create_note",
                    "description": "Create a new Markdown note with frontmatter. Auto-reindexes after write. Refuses if any tag matches a sensitive project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Relative path for the new note (e.g. notes/claude/my-note.md)" },
                            "title": { "type": "string", "description": "Note title for frontmatter" },
                            "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags for frontmatter" },
                            "summary": { "type": "string", "description": "One-line summary for quick retrieval (stored in frontmatter, indexed in DB)" },
                            "content": { "type": "string", "description": "Markdown body content (without frontmatter)" }
                        },
                        "required": ["path", "title", "summary", "content"]
                    }
                },
                {
                    "name": "append_to_note",
                    "description": "Append content to an existing note. Auto-reindexes after write. Refuses if the note's tags match a sensitive project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Relative path to existing note" },
                            "content": { "type": "string", "description": "Content to append" }
                        },
                        "required": ["path", "content"]
                    }
                },
                {
                    "name": "get_project_context",
                    "description": "Get aggregated context for a project: recent notes with summaries, key decisions, and open questions. Returns compact data suitable for session cold-start.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string", "description": "Project name (matches against tags)" },
                            "limit": { "type": "integer", "description": "Max notes to return (default 10)", "default": 10 }
                        },
                        "required": ["project"]
                    }
                }
            ]
        }
    })
}

fn handle_tool_call(
    request: &serde_json::Value,
    conn: &rusqlite::Connection,
    vault: &crate::core::vault::Vault,
) -> serde_json::Value {
    let empty = json!({});
    let params = request.get("params").unwrap_or(&empty);
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let arguments = params.get("arguments").unwrap_or(&empty);

    let result = match tool_name {
        "vault_status" => tool_vault_status(conn, vault),
        "query_notes" => tool_query_notes(conn, arguments),
        "search_notes" => tool_search_notes(conn, arguments),
        "get_note" => tool_get_note(vault, arguments),
        "check_vault" => tool_check_vault(conn),
        "create_note" => tool_create_note(conn, vault, arguments),
        "append_to_note" => tool_append_to_note(conn, vault, arguments),
        "get_project_context" => tool_get_project_context(conn, vault, arguments),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    };

    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0",
            "result": {
                "content": [{
                    "type": "text",
                    "text": content
                }]
            }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "result": {
                "content": [{
                    "type": "text",
                    "text": format!("Error: {e}")
                }],
                "isError": true
            }
        }),
    }
}

fn tool_vault_status(
    conn: &rusqlite::Connection,
    vault: &crate::core::vault::Vault,
) -> anyhow::Result<String> {
    let status = crate::core::vault::vault_status(conn, vault)?;
    Ok(serde_json::to_string_pretty(&status)?)
}

fn tool_query_notes(
    conn: &rusqlite::Connection,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    use crate::core::query::{Filter, execute_query};

    let mut filters = Vec::new();

    if let Some(tag) = args.get("tag").and_then(|t| t.as_str()) {
        filters.push(Filter::Tag(tag.to_string()));
    }
    if let Some(title) = args.get("title").and_then(|t| t.as_str()) {
        filters.push(Filter::Title(title.to_string()));
    }
    if let Some(link) = args.get("link").and_then(|l| l.as_str()) {
        filters.push(Filter::Link(link.to_string()));
    }
    if args.get("orphan").and_then(|o| o.as_bool()).unwrap_or(false) {
        filters.push(Filter::Orphan);
    }

    let results = execute_query(conn, &filters)?;
    Ok(serde_json::to_string_pretty(&results)?)
}

fn tool_search_notes(
    conn: &rusqlite::Connection,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let query = args.get("query").and_then(|q| q.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'query' argument"))?;
    if query.len() > 10_000 {
        anyhow::bail!("Query too long (max 10,000 bytes)");
    }
    let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10).min(1000) as usize;

    let results = crate::db::queries::search_fts_with_snippets(conn, query, limit)?;

    let output: Vec<serde_json::Value> = results.into_iter().map(|(path, title, snippet, rank)| {
        json!({ "path": path, "title": title, "snippet": snippet, "rank": rank })
    }).collect();

    Ok(serde_json::to_string_pretty(&output)?)
}

fn tool_get_note(
    vault: &crate::core::vault::Vault,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let path = args.get("path").and_then(|p| p.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
    let full_path = vault.root.join(path);

    // Resolve to absolute path and verify it stays within the vault root.
    // This prevents ../  traversal and symlink escapes.
    let canonical = full_path.canonicalize()
        .map_err(|_| anyhow::anyhow!("Note not found: {path}"))?;
    let canonical_root = vault.root.canonicalize()
        .map_err(|_| anyhow::anyhow!("Vault root not accessible"))?;
    if !canonical.starts_with(&canonical_root) {
        anyhow::bail!("Path escapes vault root");
    }

    let content = std::fs::read_to_string(&canonical)?;
    Ok(content)
}

fn tool_check_vault(conn: &rusqlite::Connection) -> anyhow::Result<String> {
    let broken: i64 = conn.query_row(
        "SELECT COUNT(*) FROM links WHERE target_id IS NULL", [], |row| row.get(0)
    )?;
    let orphans: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notes n
         WHERE n.id NOT IN (SELECT DISTINCT source_id FROM links WHERE source_id IS NOT NULL)
         AND n.id NOT IN (SELECT DISTINCT target_id FROM links WHERE target_id IS NOT NULL)",
        [], |row| row.get(0)
    )?;

    Ok(format!("Broken links: {broken}, Orphan notes: {orphans}"))
}

/// Check if any of the given tags match a sensitive project in the vault config.
fn check_sensitivity(vault: &crate::core::vault::Vault, tags: &[String]) -> anyhow::Result<()> {
    let sensitive = &vault.config.vault.sensitive_projects;
    if sensitive.is_empty() {
        return Ok(());
    }
    for tag in tags {
        let tag_lower = tag.to_lowercase();
        for project in sensitive {
            if tag_lower == project.to_lowercase()
                || tag_lower.starts_with(&format!("{}/", project.to_lowercase()))
            {
                anyhow::bail!(
                    "Refused: tag '{}' matches sensitive project '{}'. \
                     Cannot write notes about this project.",
                    tag, project
                );
            }
        }
    }
    Ok(())
}

fn reindex_vault(conn: &rusqlite::Connection, vault: &crate::core::vault::Vault) -> String {
    match crate::core::index::index_vault(conn, vault, false) {
        Ok(stats) => format!(
            " Reindexed: {} notes, {} links, {} tags.",
            stats.total_notes, stats.total_links, stats.total_tags
        ),
        Err(e) => format!(" Reindex failed: {e}"),
    }
}

fn tool_create_note(
    conn: &rusqlite::Connection,
    vault: &crate::core::vault::Vault,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let path = args.get("path").and_then(|p| p.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
    let title = args.get("title").and_then(|t| t.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'title' argument"))?;
    let content = args.get("content").and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;
    let summary = args.get("summary").and_then(|s| s.as_str()).unwrap_or("");

    let tags: Vec<String> = args.get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Sensitivity guard
    check_sensitivity(vault, &tags)?;

    let full_path = vault.root.join(path);

    // Prevent path traversal
    let canonical_root = vault.root.canonicalize()
        .map_err(|_| anyhow::anyhow!("Vault root not accessible"))?;
    // Canonicalize the parent since the file doesn't exist yet
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)?;
        let canonical_parent = parent.canonicalize()?;
        if !canonical_parent.starts_with(&canonical_root) {
            anyhow::bail!("Path escapes vault root");
        }
    }

    if full_path.exists() {
        anyhow::bail!("Note already exists at '{path}'. Use append_to_note to add content.");
    }

    // Build frontmatter
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let tags_yaml = if tags.is_empty() {
        String::new()
    } else {
        format!("tags: [{}]\n", tags.join(", "))
    };
    let summary_yaml = if summary.is_empty() {
        String::new()
    } else {
        format!("summary: \"{}\"\n", summary.replace('"', "'"))
    };

    let note_content = format!(
        "---\ntitle: {title}\n{tags_yaml}{summary_yaml}date: {date}\n---\n\n{content}\n"
    );

    std::fs::write(&full_path, &note_content)?;

    let mut msg = format!("Created note: {path}");
    msg.push_str(&reindex_vault(conn, vault));
    Ok(msg)
}

fn tool_append_to_note(
    conn: &rusqlite::Connection,
    vault: &crate::core::vault::Vault,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let path = args.get("path").and_then(|p| p.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
    let content = args.get("content").and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

    let full_path = vault.root.join(path);

    // Path traversal check
    let canonical = full_path.canonicalize()
        .map_err(|_| anyhow::anyhow!("Note not found: {path}"))?;
    let canonical_root = vault.root.canonicalize()
        .map_err(|_| anyhow::anyhow!("Vault root not accessible"))?;
    if !canonical.starts_with(&canonical_root) {
        anyhow::bail!("Path escapes vault root");
    }

    // Read existing content to check tags for sensitivity
    let existing = std::fs::read_to_string(&canonical)?;
    let tags = extract_frontmatter_tags(&existing);
    check_sensitivity(vault, &tags)?;

    // Append with a newline separator
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().append(true).open(&canonical)?;
    write!(file, "\n{content}\n")?;

    let mut msg = format!("Appended to note: {path}");
    msg.push_str(&reindex_vault(conn, vault));
    Ok(msg)
}

fn tool_get_project_context(
    conn: &rusqlite::Connection,
    vault: &crate::core::vault::Vault,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let project = args.get("project").and_then(|p| p.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'project' argument"))?;
    let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10).min(50) as usize;

    // Sensitivity guard: refuse context on sensitive projects
    check_sensitivity(vault, &[project.to_string()])?;

    // Query notes tagged with this project, ordered by date (from frontmatter or indexed_at)
    let mut stmt = conn.prepare(
        "SELECT n.path, n.title, n.frontmatter_json, n.indexed_at
         FROM notes n
         JOIN tags t ON t.note_id = n.id
         WHERE LOWER(t.tag) = LOWER(?1) OR LOWER(t.tag) LIKE LOWER(?2)
         GROUP BY n.id
         ORDER BY n.indexed_at DESC
         LIMIT ?3"
    )?;

    let tag_prefix = format!("{}/%", project);
    let rows = stmt.query_map(rusqlite::params![project, tag_prefix, limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut notes: Vec<serde_json::Value> = Vec::new();
    for row in rows {
        let (path, title, fm_json, indexed_at) = row?;

        // Extract summary and date from frontmatter
        let (summary, date) = if let Some(ref fm_str) = fm_json {
            let fm: serde_json::Value = serde_json::from_str(fm_str).unwrap_or_default();
            (
                fm.get("summary").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                fm.get("date").and_then(|d| d.as_str()).unwrap_or(&indexed_at).to_string(),
            )
        } else {
            (String::new(), indexed_at.clone())
        };

        notes.push(json!({
            "path": path,
            "title": title,
            "date": date,
            "summary": summary,
        }));
    }

    let output = json!({
        "project": project,
        "note_count": notes.len(),
        "notes": notes,
    });

    Ok(serde_json::to_string_pretty(&output)?)
}

/// Extract tags from YAML frontmatter (simple parser, no full YAML dep needed here).
fn extract_frontmatter_tags(content: &str) -> Vec<String> {
    let Some(rest) = content.strip_prefix("---") else { return vec![] };
    let Some(end) = rest.find("\n---") else { return vec![] };
    let frontmatter = &rest[..end];

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(tags_part) = trimmed.strip_prefix("tags:") {
            let tags_part = tags_part.trim();
            // Handle [tag1, tag2] format
            if let Some(inner) = tags_part.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                return inner.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
            }
            // Handle bare value
            if !tags_part.is_empty() {
                return vec![tags_part.to_string()];
            }
        }
    }
    vec![]
}
