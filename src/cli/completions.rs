use std::process::ExitCode;

use clap::CommandFactory;
use clap_complete::{Shell, generate};

#[derive(clap::Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    shell: Shell,
}

pub fn run(args: CompletionsArgs) -> anyhow::Result<ExitCode> {
    let mut cmd = super::Cli::command();
    generate(args.shell, &mut cmd, "oi", &mut std::io::stdout());
    Ok(ExitCode::SUCCESS)
}
