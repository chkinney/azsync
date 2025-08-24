use std::io::stdout;

use clap::CommandFactory;
use clap_complete::generate;
use clap_complete_nushell::Nushell;

use crate::{
    cli::{Cli, CompletionsOptions, GlobalOptions, Shell},
    commands::Command,
};

impl Command for CompletionsOptions {
    async fn execute(self, _global_options: &GlobalOptions) -> anyhow::Result<()> {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();

        // Map shell to clap_complete shell type
        let shell = match self.shell {
            Shell::Bash => clap_complete::Shell::Bash,
            Shell::PowerShell => clap_complete::Shell::PowerShell,
            Shell::Zsh => clap_complete::Shell::Zsh,
            Shell::Elvish => clap_complete::Shell::Elvish,
            Shell::Fish => clap_complete::Shell::Fish,
            Shell::Nushell => {
                // This uses clap_complete_nushell's generator instead
                generate(Nushell, &mut cmd, bin_name, &mut stdout());
                return Ok(());
            }
        };

        // Generate completions
        generate(shell, &mut cmd, bin_name, &mut stdout());

        Ok(())
    }
}
