//! Git adapter for the [`VcsClient`] port. Shells out to the `git` binary.
//!
//! Used by `usta new` to initialize a repository and create an initial
//! commit after a successful scaffold. Every method is best-effort at the
//! call site: the binary already wrote the project, so a missing or failing
//! `git` is surfaced as a warning, never a hard error.

use std::path::Path;
use std::process::Command;

use crate::ports::vcs::{VcsClient, VcsError};

/// Real VCS adapter backed by the system `git`.
#[derive(Debug, Default, Clone, Copy)]
pub struct GitCli;

impl GitCli {
    /// Construct.
    pub fn new() -> Self {
        Self
    }

    /// Whether `git` is on `$PATH`.
    pub fn is_available(&self) -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Run a `git` subcommand in `cwd`, mapping the result to [`VcsError`].
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<(), VcsError> {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    VcsError::NotFound("git".into())
                } else {
                    VcsError::Failed(format!("spawning git: {e}"))
                }
            })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(VcsError::Failed(format!(
                "git {}: {}",
                args.join(" "),
                stderr.trim()
            )));
        }
        Ok(())
    }
}

impl VcsClient for GitCli {
    fn init(&self, cwd: &Path) -> Result<(), VcsError> {
        // `git init -b main` to avoid the legacy `master` default and the
        // interactive "hint" noise on newer gits. `-b` is supported since
        // git 2.28 (2020); fall back to a plain init if this git is older.
        self.run(cwd, &["init", "-b", "main"])
            .or_else(|_| self.run(cwd, &["init"]))
    }

    fn add_all(&self, cwd: &Path) -> Result<(), VcsError> {
        self.run(cwd, &["add", "-A"])
    }

    fn commit(&self, cwd: &Path, message: &str) -> Result<(), VcsError> {
        // Prefer the user's configured identity. If none is set (common in
        // fresh shells / CI), retry once with a throwaway identity scoped to
        // this single commit via `-c` — it never touches global config.
        // `--no-gpg-sign` keeps a signing-enabled user from being prompted
        // (or hanging) on a scaffold's initial commit.
        let plain = ["commit", "-m", message, "--no-gpg-sign"];
        if self.run(cwd, &plain).is_ok() {
            return Ok(());
        }
        self.run(
            cwd,
            &[
                "-c",
                "user.name=usta",
                "-c",
                "user.email=usta@users.noreply.github.com",
                "commit",
                "-m",
                message,
                "--no-gpg-sign",
            ],
        )
    }
}
