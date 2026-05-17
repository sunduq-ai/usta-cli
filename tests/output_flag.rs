//! Integration tests for `usta new --output <dir>` (and the `-o` short).
//!
//! Three behaviors:
//! 1. Output to an existing directory — project lands at `<output>/<name>`.
//! 2. Output to a non-existing directory — directory is auto-created.
//! 3. Absolute output path — works the same way.
//! 4. Output path that exists as a file — clean error.

use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use tempfile::tempdir;

fn templates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .canonicalize()
        .expect("templates dir")
}

#[test]
fn output_into_existing_directory() {
    let workdir = tempdir().unwrap();
    let target_parent = workdir.path().join("projects");
    fs::create_dir_all(&target_parent).unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path()) // cwd is somewhere else entirely
        .args([
            "new",
            "explicit-output",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install", "--output"])
        .arg(&target_parent)
        .assert()
        .success();

    // Project landed at <output>/<name>, not cwd/<name>.
    assert!(target_parent.join("explicit-output").is_dir());
    assert!(!workdir.path().join("explicit-output").exists());

    // Snapshot reflects it.
    assert!(target_parent
        .join("explicit-output/.usta/snapshot.toml")
        .is_file());
}

#[test]
fn output_creates_missing_parent_directory() {
    let workdir = tempdir().unwrap();
    let nested = workdir.path().join("nope/not/yet/here");
    assert!(!nested.exists());

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "auto-mkdir",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install", "-o"]) // short flag
        .arg(&nested)
        .assert()
        .success();

    assert!(nested.is_dir(), "parent should have been auto-created");
    assert!(nested.join("auto-mkdir").is_dir());
}

#[test]
fn output_works_with_absolute_path() {
    let workdir = tempdir().unwrap();
    // tempdir().path() is already absolute on macOS/Linux/Windows.
    let absolute = workdir.path().join("abs-target");
    assert!(absolute.is_absolute() || cfg!(windows));

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir("/") // from filesystem root, prove --output is anchored
        .args([
            "new",
            "abs-app",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install", "--output"])
        .arg(&absolute)
        .assert()
        .success();

    assert!(absolute.join("abs-app").is_dir());
}

#[test]
fn output_pointing_at_a_file_errors_cleanly() {
    let workdir = tempdir().unwrap();
    let file = workdir.path().join("not-a-dir.txt");
    fs::write(&file, b"i am a file").unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "into-a-file",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install", "--output"])
        .arg(&file)
        .assert()
        .failure()
        .stderr(predicates::str::contains("not a directory"));
}

#[test]
fn dry_run_with_output_does_not_create_anything() {
    let workdir = tempdir().unwrap();
    let nested = workdir.path().join("would/not/exist");
    assert!(!nested.exists());

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "ghost-app",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--dry-run", "--no-git", "--no-install", "--output"])
        .arg(&nested)
        .assert()
        .success();

    // Critical: dry-run with --output to a non-existing path doesn't
    // pre-create the parent dir.
    assert!(!nested.exists(), "dry-run must not create --output parent");
}
