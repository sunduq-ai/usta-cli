//! Post-scaffold actions: `usta new` initializes a git repo + initial commit
//! by default, and `--no-git` opts out.
//!
//! These tests stay offline and deterministic: they always pass
//! `--no-install` (dependency install is exercised by the `pkg_manager` unit
//! tests and by manual sandbox runs, not in CI), and they only assert on the
//! git side, which needs no network.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use tempfile::tempdir;

fn templates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .canonicalize()
        .expect("templates dir")
}

fn git_available() -> bool {
    StdCommand::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn commit_count(repo: &Path) -> Option<u32> {
    let out = StdCommand::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(repo)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[test]
fn new_initializes_git_repo_by_default() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let workdir = tempdir().expect("tempdir");
    let name = "git-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["new", name, "--template", "hello-world", "--templates-dir"])
        .arg(templates_dir())
        .args(["--yes", "--no-install"]) // note: NO --no-git
        .assert()
        .success();

    let project = workdir.path().join(name);
    assert!(
        project.join(".git").is_dir(),
        "expected a .git directory after default scaffold"
    );
    assert_eq!(
        commit_count(&project),
        Some(1),
        "expected exactly one initial commit"
    );

    // The .usta/ state must be part of that commit (it's intentionally
    // tracked in generated projects).
    let tracked = StdCommand::new("git")
        .args(["ls-files"])
        .current_dir(&project)
        .output()
        .expect("git ls-files");
    let files = String::from_utf8_lossy(&tracked.stdout);
    assert!(
        files.contains(".usta/snapshot.toml"),
        "initial commit should track .usta/snapshot.toml, got:\n{files}"
    );
}

#[test]
fn no_git_flag_skips_repo_creation() {
    let workdir = tempdir().expect("tempdir");
    let name = "nogit-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["new", name, "--template", "hello-world", "--templates-dir"])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install"])
        .assert()
        .success();

    let project = workdir.path().join(name);
    assert!(
        project.join("package.json").is_file(),
        "files still written"
    );
    assert!(
        !project.join(".git").exists(),
        "--no-git must not create a repository"
    );
}

#[test]
fn scaffolding_into_an_existing_repo_does_not_clobber_it() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let workdir = tempdir().expect("tempdir");
    let name = "preexisting";
    let project = workdir.path().join(name);
    std::fs::create_dir_all(&project).unwrap();

    // Pre-create a repo with a sentinel commit.
    for args in [
        &["init", "-b", "main"][..],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--allow-empty",
            "-m",
            "sentinel",
        ][..],
    ] {
        StdCommand::new("git")
            .args(args)
            .current_dir(&project)
            .output()
            .expect("git setup");
    }

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["new", name, "--template", "hello-world", "--templates-dir"])
        .arg(templates_dir())
        .args(["--yes", "--no-install", "--force"])
        .assert()
        .success();

    // usta must not have re-init'd or added a commit over the user's repo.
    assert_eq!(
        commit_count(&project),
        Some(1),
        "existing repo's history must be left untouched (still just the sentinel)"
    );
}
