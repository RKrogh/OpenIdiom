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
                    "description": "Full-text keyword search over note content",
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

    let results = crate::db::queries::search_fts(conn, query, limit)?;

    let output: Vec<serde_json::Value> = results.into_iter().map(|(path, title, rank)| {
        json!({ "path": path, "title": title, "rank": rank })
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
