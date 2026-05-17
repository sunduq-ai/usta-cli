//! Integration tests for `usta update`.
//!
//! Scenarios:
//! 1. **Untouched files get overwritten** when the template version changes.
//! 2. **User-edited files surface as conflicts** with the proposed change
//!    written under `.usta/proposed/<path>`.
//! 3. **Re-running update against an unchanged template** is a no-op.
//! 4. **Adding a new file in the upstream template** propagates as `added`.
//! 5. **Verify is clean after a clean update** (lock matches what's on disk).

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::tempdir;

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn write_file(p: &Path, body: &[u8]) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, body).unwrap();
}

/// Seed a tempdir with a minimal template + scaffold a project from it,
/// returning (templates_dir, project_dir).
fn seed_template_and_project(workdir: &Path) -> (PathBuf, PathBuf) {
    let templates_dir = workdir.join("templates");
    let template_dir = templates_dir.join("ut");

    write_file(
        &template_dir.join("template.toml"),
        br#"
[template]
id           = "ut"
display_name = "Update Test"
version      = "0.1.0"
min_usta     = ">=0.1.0"
stacks       = []

[[features]]
id           = "core"
display_name = "Core"
default      = true
requires     = []
conflicts    = []
stacks       = []
"#,
    );
    write_file(
        &template_dir.join("base/README.md.j2"),
        b"# {{ project_name }}\n\nv1\n",
    );
    write_file(&template_dir.join("base/static.txt"), b"static-v1\n");
    write_file(
        &template_dir.join("features/core/files/note.md.j2"),
        b"core feature for {{ project_name }}\n",
    );

    // Scaffold.
    let project_dir = workdir.join("scaffolded");
    fs::create_dir_all(&project_dir).unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project_dir)
        .args(["new", "test-app", "--template", "ut", "--templates-dir"])
        .arg(&templates_dir)
        .args(["--yes", "--no-git", "--no-install"])
        .assert()
        .success();

    let project = project_dir.join("test-app");
    (templates_dir, project)
}

#[test]
fn untouched_files_get_overwritten_after_template_change() {
    let workdir = tempdir().unwrap();
    let (templates_dir, project) = seed_template_and_project(workdir.path());

    // Bump the template's README to v2.
    write_file(
        &templates_dir.join("ut/base/README.md.j2"),
        b"# {{ project_name }}\n\nv2\n",
    );

    // Update.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["update", "--templates-dir"])
        .arg(&templates_dir)
        .assert()
        .success();

    let readme = read(&project.join("README.md"));
    assert!(readme.contains("v2"), "expected v2, got:\n{readme}");
    assert!(!readme.contains("v1"));
}

#[test]
fn user_modified_files_surface_as_conflicts() {
    let workdir = tempdir().unwrap();
    let (templates_dir, project) = seed_template_and_project(workdir.path());

    // User edits README.
    write_file(
        &project.join("README.md"),
        b"# my own readme\nuser-edited\n",
    );

    // Bump template.
    write_file(
        &templates_dir.join("ut/base/README.md.j2"),
        b"# {{ project_name }}\n\nv2\n",
    );

    // Update — expect exit code 40 (conflicts present, documented in
    // ARCHITECTURE.md).
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["update", "--templates-dir"])
        .arg(&templates_dir)
        .assert()
        .code(40)
        .stdout(predicates::str::contains("conflict"))
        .stdout(predicates::str::contains("README.md"));

    // Working copy still has the user's edits.
    let readme = read(&project.join("README.md"));
    assert!(readme.contains("user-edited"));

    // Proposed file has the template's v2.
    let proposed = read(&project.join(".usta/proposed/README.md"));
    assert!(proposed.contains("v2"));
}

#[test]
fn unchanged_template_is_a_noop() {
    let workdir = tempdir().unwrap();
    let (templates_dir, project) = seed_template_and_project(workdir.path());

    // Run update without touching anything.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["update", "--templates-dir"])
        .arg(&templates_dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("0 conflicts"));

    // README content unchanged.
    let readme = read(&project.join("README.md"));
    assert!(readme.contains("v1"));
}

#[test]
fn new_template_files_are_added() {
    let workdir = tempdir().unwrap();
    let (templates_dir, project) = seed_template_and_project(workdir.path());

    // Add a new file to the template.
    write_file(
        &templates_dir.join("ut/base/CHANGELOG.md"),
        b"# Changelog\n\nv2 added me\n",
    );

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["update", "--templates-dir"])
        .arg(&templates_dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("CHANGELOG.md"));

    assert!(project.join("CHANGELOG.md").exists());
}

#[test]
fn verify_is_clean_after_clean_update() {
    let workdir = tempdir().unwrap();
    let (templates_dir, project) = seed_template_and_project(workdir.path());

    write_file(
        &templates_dir.join("ut/base/README.md.j2"),
        b"# {{ project_name }}\n\nv2\n",
    );

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["update", "--templates-dir"])
        .arg(&templates_dir)
        .assert()
        .success();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["verify"])
        .assert()
        .success() // 0 — clean
        .stdout(predicates::str::contains("no drift"));
}

#[test]
fn errors_when_not_a_usta_project() {
    let workdir = tempdir().unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["update"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("snapshot.toml"));
}
