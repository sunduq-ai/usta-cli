//! Integration test for `usta verify`.
//!
//! Scaffolds a project, runs verify (expect clean), modifies a managed file,
//! runs verify again (expect drift + exit code 41), then verifies `--json`
//! output shape.

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

fn scaffold(workdir: &std::path::Path, name: &str) -> PathBuf {
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir)
        .args(["new", name, "--template", "hello-world", "--templates-dir"])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();
    workdir.join(name)
}

#[test]
fn clean_after_fresh_scaffold() {
    let workdir = tempdir().unwrap();
    let project = scaffold(workdir.path(), "verify-clean");

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["verify"])
        .assert()
        .success() // exit 0
        .stdout(predicates::str::contains("no drift"));
}

#[test]
fn drift_detected_after_user_edit() {
    let workdir = tempdir().unwrap();
    let project = scaffold(workdir.path(), "verify-drift");

    // User edits a managed file.
    let readme = project.join("README.md");
    fs::write(&readme, "totally rewritten by user\n").unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["verify"])
        .assert()
        .code(41) // drift exit code
        .stdout(predicates::str::contains("modified"))
        .stdout(predicates::str::contains("README.md"));
}

#[test]
fn missing_file_reported() {
    let workdir = tempdir().unwrap();
    let project = scaffold(workdir.path(), "verify-missing");

    fs::remove_file(project.join("HELLO.txt")).unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["verify"])
        .assert()
        .code(41)
        .stdout(predicates::str::contains("missing"))
        .stdout(predicates::str::contains("HELLO.txt"));
}

#[test]
fn json_output_is_well_formed() {
    let workdir = tempdir().unwrap();
    let project = scaffold(workdir.path(), "verify-json");

    fs::write(project.join("README.md"), "edited").unwrap();

    let out = Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["verify", "--json"])
        .assert()
        .code(41)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(parsed["clean"], false);
    let modified = parsed["modified"].as_array().unwrap();
    assert!(modified.iter().any(|v| v.as_str() == Some("README.md")));
}

#[test]
fn errors_when_not_a_usta_project() {
    let workdir = tempdir().unwrap();

    // No .usta/ directory.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["verify"])
        .assert()
        .failure(); // generic failure (1)
}
