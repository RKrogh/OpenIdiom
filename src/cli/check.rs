use std::process::ExitCode;

use serde::Serialize;

#[derive(clap::Args)]
pub struct CheckArgs {
    /// Check for broken wikilinks
    #[arg(long)]
    broken_links: bool,

    /// Check for orphan notes
    #[arg(long)]
    orphans: bool,

    /// Check for ambiguous link resolutions
    #[arg(long)]
    ambiguous_links: bool,

    /// Check for tags used only once
    #[arg(long)]
    dead_tags: bool,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
struct CheckIssue {
    check: String,
    path: Option<String>,
    detail: String,
    line: Option<i64>,
}

pub fn run(vault_path: Option<&std::path::Path>, args: CheckArgs) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::resolve(vault_path)?;
    let conn = vault.open_db()?;

    let run_all = !args.broken_links && !args.orphans && !args.ambiguous_links && !args.dead_tags;
    let mut issues = Vec::new();

    if run_all || args.broken_links {
        issues.extend(check_broken_links(&conn)?);
    }
    if run_all || args.orphans {
        issues.extend(check_orphans(&conn)?);
    }
    if run_all || args.ambiguous_links {
        issues.extend(check_ambiguous_links(&conn)?);
    }
    if run_all || args.dead_tags {
        issues.extend(check_dead_tags(&conn)?);
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
    } else if issues.is_empty() {
        println!("No issues found");
    } else {
        for issue in &issues {
            let location = match (&issue.path, issue.line) {
                (Some(p), Some(l)) => format!("{p}:{l}"),
                (Some(p), None) => p.clone(),
                _ => String::new(),
            };
            println!("[{}] {}: {}", issue.check, location, issue.detail);
        }
    }

    if issues.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn check_broken_links(conn: &rusqlite::Connection) -> Result<Vec<CheckIssue>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT n.path, l.target_title, l.line
         FROM links l
         JOIN notes n ON n.id = l.source_id
         WHERE l.target_id IS NULL"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(CheckIssue {
            check: "broken-link".into(),
            path: Some(row.get::<_, String>(0)?),
            detail: format!("link to '{}' does not resolve to any note", row.get::<_, String>(1)?),
            line: row.get(2)?,
        })
    })?;

    rows.collect()
}

fn check_orphans(conn: &rusqlite::Connection) -> Result<Vec<CheckIssue>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT n.path, n.title FROM notes n
         WHERE n.id NOT IN (SELECT DISTINCT source_id FROM links WHERE source_id IS NOT NULL)
         AND n.id NOT IN (SELECT DISTINCT target_id FROM links WHERE target_id IS NOT NULL)"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(CheckIssue {
            check: "orphan".into(),
            path: Some(row.get::<_, String>(0)?),
            detail: format!("'{}' has no inbound or outbound links", row.get::<_, String>(1)?),
            line: None,
        })
    })?;

    rows.collect()
}

fn check_ambiguous_links(conn: &rusqlite::Connection) -> Result<Vec<CheckIssue>, rusqlite::Error> {
    // Find link targets that match multiple note titles
    let mut stmt = conn.prepare(
        "SELECT l.target_title, COUNT(DISTINCT n2.id) as cnt
         FROM links l
         JOIN notes n2 ON LOWER(n2.title) = LOWER(l.target_title)
         GROUP BY l.target_title
         HAVING cnt > 1"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(CheckIssue {
            check: "ambiguous-link".into(),
            path: None,
            detail: format!(
                "link target '{}' resolves to {} different notes",
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?
            ),
            line: None,
        })
    })?;

    rows.collect()
}

fn check_dead_tags(conn: &rusqlite::Connection) -> Result<Vec<CheckIssue>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT t.tag, COUNT(*) as cnt, n.path
         FROM tags t
         JOIN notes n ON n.id = t.note_id
         GROUP BY t.tag
         HAVING cnt = 1"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(CheckIssue {
            check: "dead-tag".into(),
            path: Some(row.get::<_, String>(2)?),
            detail: format!("tag '{}' is used only once", row.get::<_, String>(0)?),
            line: None,
        })
    })?;

    rows.collect()
}
