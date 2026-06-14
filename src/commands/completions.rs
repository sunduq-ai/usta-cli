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
use clap::Args;
use clap_complete::{generate, Shell};

#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    ///
    /// Uses `clap_complete`'s canonical names directly, so the value reads
    /// the way people actually spell it: `bash`, `zsh`, `fish`,
    /// `powershell`, `elvish`.
    pub shell: Shell,
}

/// Generate completions for the given shell from the supplied `clap::Command`.
pub fn run(args: CompletionsArgs, cmd: &mut clap::Command) -> Result<()> {
    let bin_name = cmd.get_name().to_string();
    let mut stdout = std::io::stdout();
    generate(args.shell, cmd, bin_name, &mut stdout);
    Ok(())
}
