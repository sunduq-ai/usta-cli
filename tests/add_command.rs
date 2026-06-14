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
fn add_injection_feature_post_hoc_succeeds() {
    // `new` strips every anchor marker, so the scaffolded `index.js` carries
    // no `usta:imports` marker. Adding `with-router` post-hoc must still
    // inject correctly — `add` re-renders the file from the template (which
    // has the marker) and re-applies all features, rather than depending on
    // a live marker in the user's source.
    let workdir = tempdir().unwrap();
    let project = scaffold_hello(workdir.path(), "add-inject-ok", &["greeting"]);

    let js_before = fs::read_to_string(project.join("index.js")).unwrap();
    assert!(
        !js_before.contains("usta:"),
        "no anchor marker may leak into scaffolded output:\n{js_before}"
    );
    assert!(!js_before.contains("require('./router')"));

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "with-router", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success();

    let js_after = fs::read_to_string(project.join("index.js")).unwrap();
    assert!(
        js_after.contains("const router = require('./router');"),
        "router import should be injected post-hoc:\n{js_after}"
    );
    assert!(
        !js_after.contains("usta:"),
        "no anchor marker may survive `add` either:\n{js_after}"
    );
}

#[test]
fn new_then_add_never_leak_anchor_markers() {
    // Regression for the marker-leak bug: a partial-feature scaffold used to
    // leave internal `usta:` markers in the user's source whenever the
    // optional feature targeting a marker wasn't selected. Neither `new` nor
    // a subsequent `add` may leave any marker behind.
    let workdir = tempdir().unwrap();
    let project = scaffold_hello(workdir.path(), "no-leak", &["greeting"]);

    // After scaffold: zero markers anywhere.
    let js = fs::read_to_string(project.join("index.js")).unwrap();
    assert!(!js.contains("usta:"), "scaffold leaked a marker:\n{js}");

    // After add: still zero markers, and the injection landed.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["add", "with-router", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success();

    let js = fs::read_to_string(project.join("index.js")).unwrap();
    assert!(!js.contains("usta:"), "add leaked a marker:\n{js}");
    assert!(js.contains("require('./router')"));

    // verify must still report a clean tree.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&project)
        .args(["verify"])
        .assert()
        .success()
        .stdout(predicates::str::contains("no drift"));
}
