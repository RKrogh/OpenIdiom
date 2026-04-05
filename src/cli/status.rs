use std::process::ExitCode;

pub fn run(json: bool) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::discover(&std::env::current_dir()?)?;
    let conn = vault.open_db()?;
    let status = crate::core::vault::vault_status(&conn, &vault)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("{status}");
    }

    Ok(ExitCode::SUCCESS)
}
