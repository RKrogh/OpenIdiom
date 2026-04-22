use std::path::Path;
use std::process::ExitCode;

pub fn run(vault_path: Option<&Path>, json: bool) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::resolve(vault_path)?;
    let conn = vault.open_db()?;
    let status = crate::core::vault::vault_status(&conn, &vault)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("{status}");
    }

    Ok(ExitCode::SUCCESS)
}
