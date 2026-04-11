use std::process::ExitCode;

#[derive(clap::Args)]
pub struct DailyArgs {
    /// Use yesterday's date
    #[arg(long)]
    yesterday: bool,

    /// Use a specific date (YYYY-MM-DD)
    #[arg(long)]
    date: Option<String>,

    /// Template file to use for new daily notes
    #[arg(long)]
    template: Option<String>,
}

pub fn run(vault_path: Option<&std::path::Path>, args: DailyArgs) -> anyhow::Result<ExitCode> {
    let vault = crate::core::vault::Vault::resolve(vault_path)?;

    let date = if let Some(ref date_str) = args.date {
        chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("Invalid date format: {e}. Use YYYY-MM-DD"))?
    } else if args.yesterday {
        chrono::Local::now().date_naive() - chrono::Duration::days(1)
    } else {
        chrono::Local::now().date_naive()
    };

    let filename = date.format(&vault.config.vault.daily_format).to_string();
    let rel_path = format!("{}/{}.md", vault.config.vault.daily_folder, filename);
    let full_path = vault.root.join(&rel_path);

    if !full_path.exists() {
        // Create parent dirs
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Use template if provided
        let content = if let Some(ref template_path) = args.template {
            let template_full = vault.root.join(template_path);
            std::fs::read_to_string(&template_full)
                .map_err(|e| anyhow::anyhow!("Failed to read template: {e}"))?
        } else {
            format!("# {}\n\n", date.format("%Y-%m-%d"))
        };

        std::fs::write(&full_path, content)?;
    }

    println!("{rel_path}");
    Ok(ExitCode::SUCCESS)
}
