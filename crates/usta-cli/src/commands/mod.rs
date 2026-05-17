//! Subcommand modules. Each module owns its `Args` struct and a `run` fn.
//!
//! P0 ships stubs that print "not yet implemented" with a stable exit code,
//! so `usta --help` and `--version` work end-to-end and CI can run.

pub mod add;
pub mod completions;
pub mod doctor;
pub mod extract;
pub mod install;
pub mod list;
pub mod new;
pub mod schema;
pub mod search;
pub mod self_update;
pub mod update;
pub mod verify;

use anyhow::Result;

/// Stable exit code for "stub, not implemented in this phase".
pub const EXIT_NOT_IMPLEMENTED: i32 = 64;

pub(crate) fn not_yet(phase: &str, what: &str) -> Result<()> {
    eprintln!("usta: `{what}` is implemented in {phase} (not yet wired in P0).");
    std::process::exit(EXIT_NOT_IMPLEMENTED);
}
