//! Integration tests for `usta new --dry-run` and `usta schema`.

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
fn dry_run_does_not_create_target_directory() {
    let workdir = tempdir().unwrap();
    let project = workdir.path().join("would-not-exist");
    assert!(!project.exists());

    let out = Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "would-not-exist",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--dry-run", "--no-git", "--no-install"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("dry-run"));
    assert!(text.contains("would scaffold"));
    assert!(text.contains("hello-world"));
    // Plus per-file lines.
    assert!(text.contains("+ "));

    // Critical: nothing was created.
    assert!(!project.exists(), "dry-run must not create the target dir");
}

#[test]
fn dry_run_with_merge_and_inject_features_shows_correct_kinds() {
    let workdir = tempdir().unwrap();

    let out = Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "merge-and-inject",
            "--template",
            "hello-world",
            "--features",
            "with-deps,with-router",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--dry-run", "--no-git", "--no-install"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    // with-deps merges into package.json.
    assert!(text.contains("~ "));
    assert!(text.contains("package.json"));
    // with-router injects into index.js.
    assert!(text.contains("* "));
    assert!(text.contains("index.js"));
    // The summary line.
    assert!(text.contains("merge"));
    assert!(text.contains("inject"));
}

#[test]
fn schema_template_is_valid_draft7() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["schema", "template"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(
        parsed["$schema"].as_str(),
        Some("http://json-schema.org/draft-07/schema#")
    );
    // Must declare `template` as a required object property.
    let required = parsed["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("template")));
    // The Feature definition exists and has required fields.
    let feature_def = &parsed["$defs"]["Feature"];
    assert!(feature_def["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str() == Some("id")));
}

#[test]
fn schema_feature_is_well_formed() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["schema", "feature"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(
        parsed["$schema"].as_str(),
        Some("http://json-schema.org/draft-07/schema#")
    );
    assert!(parsed["title"].as_str().unwrap().contains("feature"));
}
