use std::process::ExitCode;

use serde::Serialize;

#[derive(clap::Args)]
pub struct GraphArgs {
    /// Output format: json (default) or dot
    #[arg(long, default_value = "json")]
    format: String,

    /// Filter graph to notes with this tag
    #[arg(long)]
    filter_tag: Option<String>,

    /// Root note for ego graph
    #[arg(long)]
    root: Option<String>,

    /// Depth for ego graph (hops from root)
    #[arg(long, default_value = "2")]
    depth: usize,
}

#[derive(Serialize)]
struct Graph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
struct GraphNode {
    id: String,
    path: String,
    tags: Vec<String>,
    word_count: i64,
}

#[derive(Serialize)]
struct GraphEdge {
    source: String,
    target: String,
    line: Option<i64>,
}

pub fn run(vault_path: Option<&std::path::Path>, args: GraphArgs) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::resolve(vault_path)?;
    let conn = vault.open_db()?;

    let graph = build_graph(&conn, &args)?;

    match args.format.as_str() {
        "dot" => print_dot(&graph),
        _ => println!("{}", serde_json::to_string_pretty(&graph)?),
    }

    Ok(ExitCode::SUCCESS)
}

fn build_graph(
    conn: &rusqlite::Connection,
    args: &GraphArgs,
) -> Result<Graph, rusqlite::Error> {
    // Get all notes (optionally filtered)
    let nodes = if let Some(ref tag) = args.filter_tag {
        get_nodes_by_tag(conn, tag)?
    } else {
        get_all_nodes(conn)?
    };

    let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    // Get edges between nodes in our set
    let mut all_edges = get_all_edges(conn)?;

    // If root + depth specified, filter to ego graph
    if let Some(ref root) = args.root {
        let ego_ids = compute_ego_ids(root, &all_edges, args.depth);
        let filtered_nodes: Vec<GraphNode> = nodes
            .into_iter()
            .filter(|n| ego_ids.contains(&n.id.as_str().to_string()))
            .collect();
        all_edges.retain(|e| ego_ids.contains(&e.source) && ego_ids.contains(&e.target));
        return Ok(Graph { nodes: filtered_nodes, edges: all_edges });
    }

    // Filter edges to only those between our node set
    all_edges.retain(|e| {
        node_ids.contains(&e.source.as_str()) || node_ids.contains(&e.target.as_str())
    });

    Ok(Graph { nodes, edges: all_edges })
}

fn get_all_nodes(conn: &rusqlite::Connection) -> Result<Vec<GraphNode>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT n.title, n.path, n.word_count FROM notes n ORDER BY n.title"
    )?;

    let mut nodes = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let title: String = row.get(0)?;
        let path: String = row.get(1)?;
        let word_count: i64 = row.get(2)?;

        let id = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&title)
            .to_string();

        let tags = get_tags(conn, &path)?;
        nodes.push(GraphNode { id, path, tags, word_count });
    }
    Ok(nodes)
}

fn get_nodes_by_tag(conn: &rusqlite::Connection, tag: &str) -> Result<Vec<GraphNode>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT n.title, n.path, n.word_count
         FROM notes n JOIN tags t ON t.note_id = n.id
         WHERE t.tag = ?1
         ORDER BY n.title"
    )?;

    let mut nodes = Vec::new();
    let mut rows = stmt.query([tag])?;
    while let Some(row) = rows.next()? {
        let title: String = row.get(0)?;
        let path: String = row.get(1)?;
        let word_count: i64 = row.get(2)?;

        let id = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&title)
            .to_string();

        let tags = get_tags(conn, &path)?;
        nodes.push(GraphNode { id, path, tags, word_count });
    }
    Ok(nodes)
}

fn get_all_edges(conn: &rusqlite::Connection) -> Result<Vec<GraphEdge>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT n1.path, n2.path, l.line
         FROM links l
         JOIN notes n1 ON n1.id = l.source_id
         JOIN notes n2 ON n2.id = l.target_id"
    )?;

    let mut edges = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let source_path: String = row.get(0)?;
        let target_path: String = row.get(1)?;
        let line: Option<i64> = row.get(2)?;

        let source = std::path::Path::new(&source_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let target = std::path::Path::new(&target_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        edges.push(GraphEdge { source, target, line });
    }
    Ok(edges)
}

fn get_tags(conn: &rusqlite::Connection, path: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT t.tag FROM tags t JOIN notes n ON n.id = t.note_id WHERE n.path = ?1 ORDER BY t.tag"
    )?;
    stmt.query_map([path], |row| row.get(0))?.collect()
}

fn compute_ego_ids(root: &str, edges: &[GraphEdge], depth: usize) -> Vec<String> {
    let mut visited = vec![root.to_string()];
    let mut frontier = vec![root.to_string()];

    for _ in 0..depth {
        let mut next_frontier = Vec::new();
        for node in &frontier {
            for edge in edges {
                if edge.source == *node && !visited.contains(&edge.target) {
                    visited.push(edge.target.clone());
                    next_frontier.push(edge.target.clone());
                }
                if edge.target == *node && !visited.contains(&edge.source) {
                    visited.push(edge.source.clone());
                    next_frontier.push(edge.source.clone());
                }
            }
        }
        frontier = next_frontier;
    }

    visited
}

fn print_dot(graph: &Graph) {
    println!("digraph vault {{");
    println!("  rankdir=LR;");
    for node in &graph.nodes {
        println!("  \"{}\" [label=\"{}\"];", node.id, node.id);
    }
    for edge in &graph.edges {
        println!("  \"{}\" -> \"{}\";", edge.source, edge.target);
    }
    println!("}}");
}
