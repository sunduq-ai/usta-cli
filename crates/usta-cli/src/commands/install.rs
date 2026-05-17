//! `usta install` — install a community template into the local cache.

use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// `<github-org>/<repo>` (with optional `@<ref>` suffix).
    pub repo: String,
}

pub fn run(_args: InstallArgs) -> Result<()> {
    super::not_yet("P5", "usta install")
}
