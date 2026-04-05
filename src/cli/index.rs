use std::process::ExitCode;

pub fn run(force: bool, stats: bool) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;
    let result = crate::core::index::index_vault(&conn, &vault, force)?;

    if stats {
        println!("{result}");
    } else {
        println!(
            "Indexed {} notes, {} links, {} tags ({} new, {} updated)",
            result.total_notes, result.total_links, result.total_tags,
            result.new_notes, result.updated_notes
        );
    }

    Ok(ExitCode::SUCCESS)
}
