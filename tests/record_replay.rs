//! Integration tests for `usta new --record` and `--replay`.
//!
//! Together these enable answer-file-driven scaffolding: capture once,
//! replay deterministically in CI / regression tests / shared setups.

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
fn record_then_replay_produces_identical_tree() {
    let workdir = tempdir().unwrap();
    let answers = workdir.path().join("answers.toml");

    // Phase 1: scaffold once with --record.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "rr-first",
            "--template",
            "hello-world",
            "--features",
            "greeting,with-deps",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install", "--record"])
        .arg(&answers)
        .assert()
        .success();

    // Recorded file exists and parses as TOML.
    let recorded_text = fs::read_to_string(&answers).unwrap();
    let parsed: toml::Value = toml::from_str(&recorded_text).expect("valid TOML");
    assert_eq!(parsed["template"].as_str(), Some("hello-world"));
    let features = parsed["features"].as_array().unwrap();
    assert!(features.iter().any(|v| v.as_str() == Some("greeting")));
    assert!(features.iter().any(|v| v.as_str() == Some("with-deps")));
    let answers_map = parsed["answers"].as_table().unwrap();
    assert_eq!(
        answers_map.get("project_name").unwrap().as_str(),
        Some("rr-first")
    );

    // Phase 2: scaffold again, replaying the recorded file.
    let replay_dir = workdir.path().join("replay-target");
    fs::create_dir_all(&replay_dir).unwrap();
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&replay_dir)
        .args(["new", "rr-second", "--templates-dir"])
        .arg(templates_dir())
        .arg("--replay")
        .arg(&answers)
        .args(["--no-git", "--no-install"])
        .assert()
        .success();

    // The replayed scaffold has the same template + features applied.
    // Project name comes from `--name` (positional `rr-second`), but
    // `answers.project_name` from the file overrides it during render —
    // verify that's reflected: with-deps adds lodash, greeting adds HELLO.txt.
    let project = replay_dir.join("rr-second");
    assert!(
        project.join("HELLO.txt").is_file(),
        "greeting feature applied"
    );
    let pkg: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project.join("package.json")).unwrap()).unwrap();
    assert_eq!(pkg["dependencies"]["lodash"], "^4.17.21");

    // Snapshot records what was actually scaffolded — including the
    // recorded features (not just defaults).
    let snap = fs::read_to_string(project.join(".usta/snapshot.toml")).unwrap();
    assert!(snap.contains("with-deps"));
}

#[test]
fn replay_renders_with_recorded_answers() {
    let workdir = tempdir().unwrap();
    let answers = workdir.path().join("captured.toml");

    // Capture answers from a hello-world scaffold (which prompts for "tagline").
    // `--yes` accepts the manifest's default for tagline.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "capture-app",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install", "--record"])
        .arg(&answers)
        .assert()
        .success();

    let recorded: toml::Value = toml::from_str(&fs::read_to_string(&answers).unwrap()).unwrap();
    // The default tagline got captured.
    assert_eq!(
        recorded["answers"]["tagline"].as_str(),
        Some("An app scaffolded with usta.")
    );

    // Replay into a different project name.
    let replay_dir = workdir.path().join("rep");
    fs::create_dir_all(&replay_dir).unwrap();
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&replay_dir)
        .args(["new", "capture-app", "--templates-dir"]) // name reuses recorded
        .arg(templates_dir())
        .arg("--replay")
        .arg(&answers)
        .args(["--no-git", "--no-install"])
        .assert()
        .success();

    let readme = fs::read_to_string(replay_dir.join("capture-app/README.md")).unwrap();
    assert!(readme.contains("# capture-app"));
    assert!(readme.contains("An app scaffolded with usta."));
}

#[test]
fn replay_overrides_template_arg() {
    let workdir = tempdir().unwrap();
    let answers = workdir.path().join("hw.toml");

    // Record a hello-world scaffold.
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            "tpl-test",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .args(["--yes", "--no-git", "--no-install", "--record"])
        .arg(&answers)
        .assert()
        .success();

    // Now try replaying but pass --template nx-monorepo. The replay file
    // (hello-world) must win.
    let replay_dir = workdir.path().join("rep2");
    fs::create_dir_all(&replay_dir).unwrap();
    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&replay_dir)
        .args([
            "new",
            "should-be-hw",
            "--template",
            "nx-monorepo", // <-- intentionally wrong
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--replay")
        .arg(&answers)
        .args(["--no-git", "--no-install"])
        .assert()
        .success();

    // It should be hello-world, not nx-monorepo: HELLO.txt exists, no apps/api dir.
    let project = replay_dir.join("should-be-hw");
    assert!(project.join("HELLO.txt").is_file());
    assert!(!project.join("apps/api").exists());
    let snap = fs::read_to_string(project.join(".usta/snapshot.toml")).unwrap();
    assert!(snap.contains(r#"template_id = "hello-world""#));
}

#[test]
fn record_and_replay_are_mutually_exclusive() {
    let workdir = tempdir().unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["new", "x", "--template", "hello-world", "--templates-dir"])
        .arg(templates_dir())
        .args([
            "--yes",
            "--record",
            "/tmp/r.toml",
            "--replay",
            "/tmp/r.toml",
        ])
        .assert()
        .failure(); // clap rejects via `conflicts_with`
}
