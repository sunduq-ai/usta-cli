//! Integration test: end-to-end `usta new` against the hello-world template.
//!
//! Lives in `tests/` so it builds as a separate binary that links the
//! `usta` crate via `assert_cmd`. Uses a tempdir for the output and points
//! the binary at the in-repo `templates/` directory.

use std::path::PathBuf;

use assert_cmd::Command;
use tempfile::tempdir;

fn templates_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points at the repo root; templates live alongside.
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("templates")
        .canonicalize()
        .expect("templates dir")
}

#[test]
fn new_with_yes_creates_expected_tree() {
    let workdir = tempdir().expect("tempdir");
    let project_name = "demo-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            project_name,
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(project_name);
    assert!(project.is_dir(), "project root should exist");

    // base files
    let readme = std::fs::read_to_string(project.join("README.md")).expect("README rendered");
    assert!(readme.contains("# demo-app"));
    assert!(readme.contains("An app scaffolded with usta."));

    let agents = std::fs::read_to_string(project.join("AGENTS.md")).expect("AGENTS rendered");
    assert!(agents.contains("# AGENTS.md — demo-app"));

    // .gitignore copied verbatim
    assert!(project.join(".gitignore").is_file());

    // greeting feature is `default = true` → HELLO.txt should be there
    let hello = std::fs::read_to_string(project.join("HELLO.txt")).expect("HELLO rendered");
    assert!(hello.contains("Hello from demo-app!"));

    // license-mit is `default = false` and we didn't pass --features → should NOT be there
    assert!(
        !project.join("LICENSE").exists(),
        "LICENSE should not be present without --features"
    );

    // Snapshot files must be present.
    let snap = std::fs::read_to_string(project.join(".usta/snapshot.toml"))
        .expect("snapshot.toml present");
    assert!(snap.contains("template_id"));
    assert!(snap.contains("hello-world"));
    assert!(snap.contains("created_at"));

    let lock = std::fs::read_to_string(project.join(".usta/managed.lock")).expect("lock present");
    assert!(lock.lines().count() >= 4); // header + 3 base files + 1 feature file
    assert!(lock.contains("README.md"));
}

#[test]
fn new_with_explicit_features_includes_license() {
    let workdir = tempdir().expect("tempdir");
    let project_name = "with-license";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            project_name,
            "--template",
            "hello-world",
            "--features",
            "greeting,license-mit",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(project_name);
    assert!(project.join("LICENSE").is_file());
    assert!(project.join("HELLO.txt").is_file());
}

#[test]
fn merge_adds_dependency_to_package_json() {
    let workdir = tempdir().expect("tempdir");
    let project_name = "merge-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            project_name,
            "--template",
            "hello-world",
            "--features",
            "with-deps",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(project_name);
    let pkg = std::fs::read_to_string(project.join("package.json")).expect("package.json");
    let parsed: serde_json::Value = serde_json::from_str(&pkg).expect("valid JSON");

    // base values preserved
    assert_eq!(parsed["name"], "@merge-app/merge-app");
    assert_eq!(parsed["version"], "0.0.1");
    assert_eq!(parsed["scripts"]["start"], "node index.js");

    // merged values present
    assert_eq!(parsed["dependencies"]["lodash"], "^4.17.21");
    assert_eq!(parsed["scripts"]["test"], "echo 'no tests yet'");
}

#[test]
fn injection_inserts_import_into_anchor() {
    let workdir = tempdir().expect("tempdir");
    let project_name = "inject-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            project_name,
            "--template",
            "hello-world",
            "--features",
            "with-router",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(project_name);
    let js = std::fs::read_to_string(project.join("index.js")).expect("index.js");
    assert!(
        js.contains("const router = require('./router');"),
        "expected injected import line, got:\n{js}"
    );
    // Marker must be stripped from the output.
    assert!(
        !js.contains("usta:imports"),
        "marker leaked into output:\n{js}"
    );
    // Regression: the base `index.js` carries `{{ project_name }}` and must
    // be rendered (it lives at `base/index.js.j2`), not copied verbatim.
    // A past bug shipped it as `base/index.js`, so `node index.js` printed
    // the literal `{{ project_name }}`.
    assert!(
        js.contains("hello from inject-app") && !js.contains("{{"),
        "index.js must interpolate project_name, got:\n{js}"
    );
    // Regression: the `with-router` feature injects `require('./router')`,
    // so it must ship `router.js` or the project crashes at runtime.
    assert!(
        project.join("router.js").is_file(),
        "with-router must ship router.js for the injected require to resolve"
    );
}

#[test]
fn rejects_invalid_project_name() {
    let workdir = tempdir().expect("tempdir");
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "BadName",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .failure();
}
