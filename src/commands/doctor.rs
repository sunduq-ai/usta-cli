//! `usta doctor` — check the local environment for everything `usta`'s
//! templates and post-scaffold hooks expect.
//!
//! For each tool we report `present` (with version output trimmed) or
//! `missing`. The command does not fail if a tool is missing; it reports.
//! `--strict` flips that to exit non-zero on any missing tool.

use std::process::Command as Cmd;

use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Exit non-zero if any tool is missing (default: succeed regardless,
    /// just report).
    #[arg(long)]
    pub strict: bool,
}

/// One tool we expect on `$PATH`.
struct ToolCheck {
    /// Display name (e.g. "git").
    name: &'static str,
    /// Argument that asks for a version (usually `--version`).
    version_arg: &'static str,
    /// Why this tool matters; shown when missing.
    why: &'static str,
}

const CHECKS: &[ToolCheck] = &[
    ToolCheck {
        name: "git",
        version_arg: "--version",
        why: "scaffolds initialize a git repo (skip with --no-git)",
    },
    ToolCheck {
        name: "node",
        version_arg: "--version",
        why: "TypeScript / React / mobile templates",
    },
    ToolCheck {
        name: "pnpm",
        version_arg: "--version",
        why: "the nx-monorepo template's package manager",
    },
    ToolCheck {
        name: "npm",
        version_arg: "--version",
        why: "fallback for templates that prefer npm",
    },
    ToolCheck {
        name: "uv",
        version_arg: "--version",
        why: "Python package management for FastAPI templates",
    },
    ToolCheck {
        name: "python3",
        version_arg: "--version",
        why: "Python templates",
    },
    ToolCheck {
        name: "cargo",
        version_arg: "--version",
        why: "Rust templates (and rebuilding usta from source)",
    },
    ToolCheck {
        name: "go",
        version_arg: "version",
        why: "Go templates (planned)",
    },
    ToolCheck {
        name: "docker",
        version_arg: "--version",
        why: "the docker feature",
    },
];

#[derive(Debug)]
struct Result1 {
    name: &'static str,
    version: Option<String>,
    why: &'static str,
}

pub fn run(args: DoctorArgs) -> Result<()> {
    let mut results: Vec<Result1> = Vec::with_capacity(CHECKS.len());
    for check in CHECKS {
        let version = capture_version(check.name, check.version_arg);
        results.push(Result1 {
            name: check.name,
            version,
            why: check.why,
        });
    }

    let any_missing = results.iter().any(|r| r.version.is_none());

    if args.json {
        let payload = serde_json::json!({
            "tools": results.iter().map(|r| serde_json::json!({
                "name": r.name,
                "present": r.version.is_some(),
                "version": r.version,
                "why": r.why,
            })).collect::<Vec<_>>(),
            "all_present": !any_missing,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("usta doctor — environment check");
        for r in &results {
            match &r.version {
                Some(v) => println!("  ✓ {:<10} {}", r.name, v),
                None => println!("  ✗ {:<10} (missing — {})", r.name, r.why),
            }
        }
        if !any_missing {
            println!("✓ all expected tools present");
        }
    }

    if args.strict && any_missing {
        std::process::exit(1);
    }
    Ok(())
}

/// Run `<bin> <version_arg>` and return the first non-empty line of stdout
/// (trimmed). Returns `None` if the binary can't be invoked.
fn capture_version(bin: &str, version_arg: &str) -> Option<String> {
    let out = Cmd::new(bin).arg(version_arg).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
}
