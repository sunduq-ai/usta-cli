//! Integration tests for `usta list`, `usta completions`, and
//! `usta doctor` — the discovery / introspection / env commands.

use std::path::PathBuf;

use assert_cmd::Command;

fn templates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .canonicalize()
        .expect("templates dir")
}

// ─────────────────────────── usta list templates ────────────────────────

#[test]
fn list_templates_shows_known_ids() {
    Command::cargo_bin("usta")
        .expect("binary")
        .args(["list", "templates", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success()
        .stdout(predicates::str::contains("hello-world"))
        .stdout(predicates::str::contains("nx-monorepo"));
}

#[test]
fn list_templates_json_is_well_formed() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["list", "templates", "--json", "--templates-dir"])
        .arg(templates_dir())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    let arr = parsed.as_array().expect("array");
    assert!(arr.iter().any(|t| t["id"].as_str() == Some("hello-world")));
    assert!(arr.iter().any(|t| t["id"].as_str() == Some("nx-monorepo")));
    // Each entry has the required shape.
    for entry in arr {
        assert!(entry["id"].is_string());
        assert!(entry["display_name"].is_string());
        assert!(entry["version"].is_string());
        assert!(entry["features"].is_number());
    }
}

// ─────────────────────────── usta list features ─────────────────────────

#[test]
fn list_features_for_known_template() {
    Command::cargo_bin("usta")
        .expect("binary")
        .args([
            "list",
            "features",
            "--template",
            "hello-world",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .assert()
        .success()
        .stdout(predicates::str::contains("greeting"))
        .stdout(predicates::str::contains("license-mit"));
}

#[test]
fn list_features_json_carries_dependencies() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args([
            "list",
            "features",
            "--template",
            "nx-monorepo",
            "--json",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    let arr = parsed.as_array().expect("array");
    // api-mongodb requires api-fastapi.
    let mongo = arr
        .iter()
        .find(|f| f["id"].as_str() == Some("api-mongodb"))
        .expect("api-mongodb present");
    let requires = mongo["requires"].as_array().unwrap();
    assert!(requires.iter().any(|r| r.as_str() == Some("api-fastapi")));
}

#[test]
fn list_features_unknown_template_errors() {
    Command::cargo_bin("usta")
        .expect("binary")
        .args([
            "list",
            "features",
            "--template",
            "totally-fake",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .assert()
        .failure();
}

// ─────────────────────────── usta completions ───────────────────────────

#[test]
fn completions_bash_emits_function() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["completions", "bash"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    // bash completions reference the binary name and use a complete function.
    assert!(text.contains("usta"));
    assert!(text.contains("complete"));
}

#[test]
fn completions_zsh_emits_compdef() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["completions", "zsh"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("#compdef usta"));
}

#[test]
fn completions_fish_emits_complete_directives() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["completions", "fish"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("complete -c usta"));
}

#[test]
fn completions_accept_canonical_shell_names() {
    // Regression: a custom `Shell` ValueEnum had clap kebab-case the
    // `PowerShell` variant to `power-shell`, so the obvious `powershell`
    // spelling was rejected. We now use `clap_complete::Shell` directly,
    // whose value is the canonical one-word `powershell`. Every shell name
    // a user would actually type must be accepted.
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        Command::cargo_bin("usta")
            .expect("binary")
            .args(["completions", shell])
            .assert()
            .success();
    }
}

// ─────────────────────────── usta doctor ───────────────────────────────

#[test]
fn doctor_reports_some_tools() {
    // The runtime has `cargo` (test binary builds with cargo) and `git`
    // (integration tests run in CI which always has git). Don't be too
    // picky about which other tools are present — just verify the output
    // mentions at least one tool we know is present.
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("cargo"));
    assert!(text.contains("git"));
}

#[test]
fn doctor_json_is_well_formed() {
    let out = Command::cargo_bin("usta")
        .expect("binary")
        .args(["doctor", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert!(parsed["tools"].is_array());
    assert!(parsed["all_present"].is_boolean());
    let tools = parsed["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"].as_str() == Some("cargo")));
}
