use std::process::ExitCode;

pub fn run() -> anyhow::Result<ExitCode> {
    let vault_dir = std::env::current_dir()?;
    crate::core::vault::init_vault(&vault_dir)?;
    println!("Initialized vault in {}", vault_dir.display());
    Ok(ExitCode::SUCCESS)
}
