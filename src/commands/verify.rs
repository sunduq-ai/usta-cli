//! `usta verify` — detect drift in template-managed files.
//!
//! Reads `.usta/managed.lock` from the current directory (or `--cwd`),
//! re-hashes each managed file, and reports modified / missing entries.
//!
//! Exit codes (per `docs/ARCHITECTURE.md`):
//! - 0   — no drift detected.
//! - 41  — drift detected (modified or missing files).
//! - 1   — generic failure (e.g. lock file malformed).

use std::path::PathBuf;

use crate::adapters::fs::LocalFs;
use crate::app::verify;
use anyhow::{anyhow, Context, Result};
use clap::Args;

#[derive(Debug, Args)]
pub struct VerifyArgs {
    /// Project root (defaults to current directory).
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Emit machine-readable JSON (paths under `unchanged`/`modified`/`missing`).
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: VerifyArgs) -> Result<()> {
    let project_root = args
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
        .canonicalize()
        .context("resolving project root")?;
    if !project_root.is_dir() {
        return Err(anyhow!(
            "project root is not a directory: {}",
            project_root.display()
        ));
    }

    let fs = LocalFs::new(&project_root);
    let report = verify::verify(&fs).map_err(|e| anyhow!("verify failed: {e}"))?;

    if args.json {
        let payload = serde_json::json!({
            "unchanged": report.unchanged.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "modified":  report.modified.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "missing":   report.missing.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "clean":     report.is_clean(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "{} managed files: {} unchanged, {} modified, {} missing.",
            report.total(),
            report.unchanged.len(),
            report.modified.len(),
            report.missing.len()
        );
        for p in &report.modified {
            println!("  ! modified  {}", p.display());
        }
        for p in &report.missing {
            println!("  ✗ missing   {}", p.display());
        }
        if report.is_clean() {
            println!("✓ no drift");
        }
    }

    if !report.is_clean() {
        std::process::exit(41);
    }
    Ok(())
}
