use std::path::Path;
use std::process::ExitCode;

pub fn run(vault_path: Option<&Path>) -> anyhow::Result<ExitCode> {
    let vault_dir = match vault_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    crate::core::vault::init_vault(&vault_dir)?;
    println!("Initialized vault in {}", vault_dir.display());
    Ok(ExitCode::SUCCESS)
}
