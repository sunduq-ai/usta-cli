//! Integration tests for `usta add`.
//!
//! Scenarios:
//! 1. Scaffold with one feature, add another that's purely additive (only
//!    Write ops + new files) → succeeds.
//! 2. Scaffold without a Merge-only feature, add it → root config file
//!    deep-merges.
//! 3. Try to add a feature that's already applied → error.
//! 4. Try to add an unknown feature → error.
//! 5. Verify after add → still clean (lock includes new files).
//! 6. Adding a feature whose contributions inject into a previously-finalized
//!    anchor surfaces the documented `AnchorMarkerMissing`-style error.

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

fn scaffold_hello(workdir: &std::path::Path, name: &str, features: &[&str]) -> PathBuf {
    let mut cmd = Command::cargo_bin("usta").expect("binary");
    cmd.current_dir(workdir)
        .args(["new", name, "--template", "hello-world", "--templates-dir"]);
    cmd.arg(templates_dir());
    if !features.is_empty() {
        cmd.args(["--features", &features.join(",")]);
    }
    cmd.args(["--yes", "--no-git", "--no-install"]);
    cmd.assert().success();
    workdir.join(name)
}

#[test]
fn add_appends_new_files_and_updates_lock() {
    let workdir = tempdir().unwrap();
    // Scaffold WITHOUT license-mit.
    let project = scaffold_hello(workdir.path(), "add-files", &["greeting"]);
    assert!(!project.join("LICENSE").exists());

    // Add license-mit (pure file feature, no inject).
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "license-mit", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success();

    // The new file is present.
    let license = fs::read_to_string(project.join("LICENSE")).unwrap();
    assert!(license.contains("MIT License"));
    assert!(license.contains("add-files"));

    // Snapshot now includes both features.
    let snap = fs::read_to_string(project.join(".usta/snapshot.toml")).unwrap();
    assert!(snap.contains("greeting"));
    assert!(snap.contains("license-mit"));

    // Lock now includes the new file.
    let lock = fs::read_to_string(project.join(".usta/managed.lock")).unwrap();
    assert!(lock.contains("LICENSE"));
}

#[test]
fn add_then_verify_is_clean() {
    let workdir = tempdir().unwrap();
    let project = scaffold_hello(workdir.path(), "add-then-verify", &["greeting"]);

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "license-mit", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["verify"])
        .assert()
        .success() // exit 0 — clean
        .stdout(predicates::str::contains("no drift"));
}

#[test]
fn add_merge_only_feature_deep_merges() {
    let workdir = tempdir().unwrap();
    // Scaffold without with-deps (it's not a default).
    let project = scaffold_hello(workdir.path(), "add-merge", &["greeting"]);

    let pkg_before = fs::read_to_string(project.join("package.json")).unwrap();
    assert!(!pkg_before.contains("lodash"));

    // Add the merge-only feature.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "with-deps", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success();

    // package.json now has lodash AND retains its base values.
    let pkg_after = fs::read_to_string(project.join("package.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&pkg_after).expect("valid JSON");
    assert_eq!(parsed["dependencies"]["lodash"], "^4.17.21");
    assert_eq!(parsed["scripts"]["start"], "node index.js"); // base preserved
}

#[test]
fn adding_already_applied_feature_errors() {
    let workdir = tempdir().unwrap();
    let project = scaffold_hello(workdir.path(), "already-applied", &["greeting"]);

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "greeting", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .failure()
        .stderr(predicates::str::contains("already applied"));
}

#[test]
fn adding_unknown_feature_errors() {
    let workdir = tempdir().unwrap();
    let project = scaffold_hello(workdir.path(), "unknown-feature", &["greeting"]);

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "totally-fake", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown feature"));
}

#[test]
fn add_inject_when_marker_still_present_succeeds() {
    // Scaffold without `with-router`: nothing has consumed the
    // `usta:imports` marker yet, so it's still in `index.js`. Adding
    // `with-router` post-hoc should successfully inject.
    let workdir = tempdir().unwrap();
    let project = scaffold_hello(workdir.path(), "add-inject-ok", &["greeting"]);

    let js_before = fs::read_to_string(project.join("index.js")).unwrap();
    assert!(
        js_before.contains("usta:imports"),
        "marker should still be present when no feature consumed it"
    );

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "with-router", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success();

    let js_after = fs::read_to_string(project.join("index.js")).unwrap();
    assert!(js_after.contains("const router = require('./router');"));
    assert!(!js_after.contains("usta:imports"));
}

#[test]
fn anchor_inject_post_hoc_surfaces_helpful_error_when_marker_gone() {
    // Simulate a prior feature having consumed the marker by manually
    // stripping it. Adding a feature that wants to inject into it should
    // surface a helpful "use `usta update`" error.
    let workdir = tempdir().unwrap();
    let project = scaffold_hello(workdir.path(), "inject-after-gone", &["greeting"]);

    // Manually remove the marker line.
    let js_path = project.join("index.js");
    let content = fs::read_to_string(&js_path).unwrap();
    let stripped: String = content
        .lines()
        .filter(|l| !l.contains("usta:imports"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&js_path, stripped).unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "with-router", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .failure()
        .stderr(predicates::str::contains("usta update"))
        .stderr(predicates::str::contains("usta:imports"));
}
