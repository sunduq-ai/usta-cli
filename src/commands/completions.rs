//! `usta completions <shell>` — emit shell completions to stdout.
//!
//! Pipe to your shell's completion location, e.g.:
//!
//! ```bash
//! usta completions zsh > "${fpath[1]}/_usta"
//! usta completions bash > /etc/bash_completion.d/usta
//! usta completions fish > ~/.config/fish/completions/usta.fish
//! ```

use anyhow::Result;
use clap::{Args, ValueEnum};
use clap_complete::{generate, Shell as ClapShell};

#[derive(Debug, Clone, Copy, ValueEnum)]
#[allow(clippy::enum_variant_names)]
pub enum Shell {
    /// Bash.
    Bash,
    /// Zsh.
    Zsh,
    /// Fish.
    Fish,
    /// PowerShell.
    PowerShell,
    /// Elvish.
    Elvish,
}

impl From<Shell> for ClapShell {
    fn from(s: Shell) -> Self {
        match s {
            Shell::Bash => ClapShell::Bash,
            Shell::Zsh => ClapShell::Zsh,
            Shell::Fish => ClapShell::Fish,
            Shell::PowerShell => ClapShell::PowerShell,
            Shell::Elvish => ClapShell::Elvish,
        }
    }
}

#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    pub shell: Shell,
}

/// Generate completions for the given shell from the supplied `clap::Command`.
pub fn run(args: CompletionsArgs, cmd: &mut clap::Command) -> Result<()> {
    let shell: ClapShell = args.shell.into();
    let bin_name = cmd.get_name().to_string();
    let mut stdout = std::io::stdout();
    generate(shell, cmd, bin_name, &mut stdout);
    Ok(())
}
