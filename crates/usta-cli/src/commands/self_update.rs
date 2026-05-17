//! `usta self-update` — replace this binary with the latest release.

use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct SelfUpdateArgs {
    /// Pin a specific release version (default: latest).
    /// Renamed from `--version` so it doesn't collide with the global
    /// `--version` flag clap auto-generates per subcommand.
    #[arg(long = "release", value_name = "VERSION")]
    pub release: Option<String>,

    /// Skip the confirmation prompt.
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub fn run(_args: SelfUpdateArgs) -> Result<()> {
    super::not_yet("P5", "usta self-update")
}
