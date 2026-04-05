use std::process::ExitCode;

use serde::Serialize;

#[derive(clap::Args)]
pub struct SearchArgs {
    /// Search query
    query: String,

    /// Maximum results
    #[arg(long, default_value = "10")]
    limit: usize,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Output only file paths
    #[arg(long)]
    paths: bool,
}

#[derive(Serialize)]
struct SearchResult {
    path: String,
    title: String,
    rank: f64,
}

pub fn run(args: SearchArgs) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;

    let results = crate::db::queries::search_fts(&conn, &args.query, args.limit)?;

    let search_results: Vec<SearchResult> = results
        .into_iter()
        .map(|(path, title, rank)| SearchResult { path, title, rank })
        .collect();

    if args.json {
        println!("{}", serde_json::to_string_pretty(&search_results)?);
    } else if args.paths {
        for r in &search_results {
            println!("{}", r.path);
        }
    } else if search_results.is_empty() {
        println!("No results");
    } else {
        use comfy_table::{Table, ContentArrangement};
        let mut table = Table::new();
        table.set_content_arrangement(ContentArrangement::Dynamic);
        table.set_header(vec!["Title", "Path", "Relevance"]);
        for r in &search_results {
            table.add_row(vec![
                &r.title,
                &r.path,
                &format!("{:.2}", r.rank),
            ]);
        }
        println!("{table}");
    }

    Ok(ExitCode::SUCCESS)
}
