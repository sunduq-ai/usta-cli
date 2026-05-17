//! `usta search` — discover community templates via GitHub topic.

use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Search query.
    pub query: String,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

pub fn run(_args: SearchArgs) -> Result<()> {
    super::not_yet("P5", "usta search")
}
