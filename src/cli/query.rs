use std::process::ExitCode;

use crate::core::query::{Filter, QueryResult, execute_query};

#[derive(clap::Args)]
pub struct QueryArgs {
    /// Filter by tag (repeatable, AND logic)
    #[arg(long)]
    tag: Vec<String>,

    /// Notes that link TO this target
    #[arg(long)]
    link: Option<String>,

    /// Notes that this source links TO
    #[arg(long)]
    backlink: Option<String>,

    /// Title contains this string
    #[arg(long)]
    title: Option<String>,

    /// Frontmatter field match (key=value)
    #[arg(long)]
    front: Option<String>,

    /// Minimum word count
    #[arg(long)]
    min_words: Option<usize>,

    /// Only orphan notes (no links in or out)
    #[arg(long)]
    orphan: bool,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Output only file paths
    #[arg(long)]
    paths: bool,
}

pub fn run(vault_path: Option<&std::path::Path>, args: QueryArgs) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::resolve(vault_path)?;
    let conn = vault.open_db()?;

    let mut filters = Vec::new();

    for tag in &args.tag {
        filters.push(Filter::Tag(tag.clone()));
    }
    if let Some(ref link) = args.link {
        filters.push(Filter::Link(link.clone()));
    }
    if let Some(ref backlink) = args.backlink {
        filters.push(Filter::Backlink(backlink.clone()));
    }
    if let Some(ref title) = args.title {
        filters.push(Filter::Title(title.clone()));
    }
    if let Some(ref front) = args.front {
        if let Some((key, value)) = front.split_once('=') {
            filters.push(Filter::Frontmatter(key.to_string(), value.to_string()));
        } else {
            anyhow::bail!("--front must be in key=value format");
        }
    }
    if let Some(min) = args.min_words {
        filters.push(Filter::MinWords(min));
    }
    if args.orphan {
        filters.push(Filter::Orphan);
    }

    let results = execute_query(&conn, &filters)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if args.paths {
        for r in &results {
            println!("{}", r.path);
        }
    } else if results.is_empty() {
        println!("No results");
    } else {
        print_table(&results);
    }

    Ok(ExitCode::SUCCESS)
}

fn print_table(results: &[QueryResult]) {
    use comfy_table::{Table, ContentArrangement};

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Title", "Path", "Words", "Tags"]);

    for r in results {
        table.add_row(vec![
            &r.title,
            &r.path,
            &r.word_count.to_string(),
            &r.tags.join(", "),
        ]);
    }

    println!("{table}");
}
