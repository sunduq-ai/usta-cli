//! Integration test for the `usta extract` pipeline.
//!
//! Builds a synthetic fixture repo in a tempdir, runs `usta extract`, and
//! asserts:
//! - the synthesized template tree exists with expected files
//! - `template.toml` is valid and parses to the expected manifest
//! - identifier substitution + `.j2` renaming worked
//! - default-noise files (e.g. `node_modules/`, lockfiles) were dropped
//! - feature partitioning routed files correctly
//! - **round-trip**: `usta new` against the synthesized template scaffolds
//!   a sensible project (proves extract → scaffold loop works)

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::tempdir;

fn write(p: &Path, body: &[u8]) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, body).unwrap();
}

/// Build a small synthetic repo and a `.usta-extract.toml` for it.
fn fixture_repo(root: &Path) {
    // Source files we want to keep.
    write(
        &root.join("README.md"),
        b"# my-existing-app\n\nMyExistingApp is a sample.\n",
    );
    write(
        &root.join("package.json"),
        br#"{"name": "my-existing-app", "private": true}"#,
    );
    write(
        &root.join("apps/api/main.py"),
        b"# my-existing-app API\nprint('hello from my-existing-app')\n",
    );
    write(
        &root.join("apps/web/src/App.tsx"),
        b"export default function App() { return <h1>my-existing-app</h1>; }\n",
    );

    // Defaults that should be dropped.
    write(&root.join("node_modules/x/index.js"), b"junk");
    write(&root.join("apps/web/dist/bundle.js"), b"junk");
    write(&root.join("pnpm-lock.yaml"), b"lockfile");
    write(&root.join("apps/api/__pycache__/x.pyc"), b"\x00\x00\x00");
    write(&root.join(".DS_Store"), b"\x00\x00");

    // .gitignore (also handled by ignore walker).
    write(&root.join(".gitignore"), b"*.local\nsecret.txt\n");
    write(&root.join("secret.txt"), b"shhh");

    // Extract config.
    write(
        &root.join(".usta-extract.toml"),
        br#"
template_id          = "my-existing-app-shape"
template_display_name = "MyExistingApp Shape"
stacks               = ["typescript", "python"]

[identifiers]
"my-existing-app" = "{{ project_name }}"
"MyExistingApp" = "{{ project_name | pascal }}"

[[features]]
id           = "api"
display_name = "API"
default      = true
paths        = ["apps/api/**"]

[[features]]
id           = "web"
display_name = "Web"
default      = true
paths        = ["apps/web/**"]
"#,
    );
}

#[test]
fn extracts_synthetic_repo_into_well_formed_template() {
    let workdir = tempdir().expect("tempdir");
    let repo = workdir.path().join("source-repo");
    fs::create_dir_all(&repo).unwrap();
    fixture_repo(&repo);

    let out_dir = workdir.path().join("out-templates");

    Command::cargo_bin("usta")
        .expect("binary")
        .args(["extract"])
        .arg(&repo)
        .arg("--out")
        .arg(&out_dir)
        .arg("--force")
        .assert()
        .success();

    let template_dir = out_dir.join("my-existing-app-shape");
    assert!(
        template_dir.is_dir(),
        "synthesized template dir not found: {}",
        template_dir.display()
    );

    // Manifest is valid TOML and parses.
    let manifest_text = fs::read_to_string(template_dir.join("template.toml")).unwrap();
    let manifest: toml::Value = toml::from_str(&manifest_text).expect("valid template.toml");
    assert_eq!(
        manifest["template"]["id"].as_str(),
        Some("my-existing-app-shape")
    );
    assert_eq!(
        manifest["template"]["display_name"].as_str(),
        Some("MyExistingApp Shape")
    );
    let stacks = manifest["template"]["stacks"].as_array().unwrap();
    assert!(stacks.iter().any(|v| v.as_str() == Some("typescript")));
    assert!(stacks.iter().any(|v| v.as_str() == Some("python")));

    // Default-noise files were dropped.
    assert!(!template_dir.join("base/node_modules").exists());
    assert!(!template_dir.join("base/pnpm-lock.yaml").exists());
    assert!(!template_dir.join("base/apps/api/__pycache__").exists());
    assert!(!template_dir.join("base/.DS_Store").exists());
    // .gitignore-listed file dropped by the scanner.
    assert!(!template_dir.join("base/secret.txt").exists());

    // README had identifier replaced → got `.j2` suffix and rendered text.
    // Fixture: "# my-existing-app\n\nMyExistingApp is a sample.\n"
    // Substitutions: my-existing-app → {{ project_name }}, MyExistingApp → {{ project_name | pascal }}.
    let readme = fs::read_to_string(template_dir.join("base/README.md.j2"))
        .expect("README.md.j2 should exist (substituted)");
    assert!(
        readme.contains("# {{ project_name }}"),
        "expected lowercase title substituted, got:\n{readme}"
    );
    assert!(readme.contains("{{ project_name | pascal }} is a sample."));

    // package.json substituted → .j2.
    let pkg = fs::read_to_string(template_dir.join("base/package.json.j2"))
        .expect("package.json.j2 should exist");
    assert!(pkg.contains(r#""name": "{{ project_name }}""#));

    // Files routed to features by glob. Fixture used all lowercase
    // identifiers in these files, so the substitution is `{{ project_name }}`.
    let api_main =
        fs::read_to_string(template_dir.join("features/api/files/apps/api/main.py.j2")).unwrap();
    assert!(api_main.contains("# {{ project_name }} API"));
    assert!(api_main.contains("hello from {{ project_name }}"));
    let web_app =
        fs::read_to_string(template_dir.join("features/web/files/apps/web/src/App.tsx.j2"))
            .unwrap();
    assert!(web_app.contains("<h1>{{ project_name }}</h1>"));

    // Manifest declares both features.
    let features = manifest["features"].as_array().unwrap();
    assert!(features.iter().any(|f| f["id"].as_str() == Some("api")));
    assert!(features.iter().any(|f| f["id"].as_str() == Some("web")));
}

#[test]
fn extract_then_scaffold_round_trip() {
    let workdir = tempdir().expect("tempdir");
    let repo = workdir.path().join("source-repo");
    fs::create_dir_all(&repo).unwrap();
    fixture_repo(&repo);

    let out_dir = workdir.path().join("out-templates");

    // Step 1: extract.
    Command::cargo_bin("usta")
        .expect("binary")
        .args(["extract"])
        .arg(&repo)
        .arg("--out")
        .arg(&out_dir)
        .arg("--force")
        .assert()
        .success();

    // Step 2: scaffold from the synthesized template into a fresh project.
    let scaffold_dir = workdir.path().join("scaffold-target");
    fs::create_dir_all(&scaffold_dir).unwrap();

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(&scaffold_dir)
        .args([
            "new",
            "round-trip",
            "--template",
            "my-existing-app-shape",
            "--templates-dir",
        ])
        .arg(&out_dir)
        .args(["--features", "api,web", "--yes", "--no-git", "--no-install"])
        .assert()
        .success();

    let project = scaffold_dir.join("round-trip");

    // Identifier substitution propagated through the round-trip:
    // - source files used lowercase "my-existing-app" in title/body of README's first
    //   line, in API comment, in console.log, and in JSX → these become
    //   `{{ project_name }}` → render to "round-trip".
    // - source README's body had "MyExistingApp" → became `{{ project_name | pascal }}`
    //   → renders to "RoundTrip".
    let readme = fs::read_to_string(project.join("README.md")).unwrap();
    assert!(
        readme.contains("# round-trip"),
        "round-trip title not rendered:\n{readme}"
    );
    assert!(
        readme.contains("RoundTrip is a sample."),
        "expected pascal substitution to render to RoundTrip:\n{readme}"
    );

    let api_main = fs::read_to_string(project.join("apps/api/main.py")).unwrap();
    assert!(api_main.contains("# round-trip API"));
    assert!(api_main.contains("hello from round-trip"));

    let web_app = fs::read_to_string(project.join("apps/web/src/App.tsx")).unwrap();
    assert!(web_app.contains("<h1>round-trip</h1>"));

    // Snapshot from the round-trip records the synthesized template id.
    let snap = fs::read_to_string(project.join(".usta/snapshot.toml")).unwrap();
    assert!(snap.contains(r#"template_id = "my-existing-app-shape""#));
}

#[test]
fn extract_is_deterministic() {
    let workdir = tempdir().expect("tempdir");
    let repo = workdir.path().join("source-repo");
    fs::create_dir_all(&repo).unwrap();
    fixture_repo(&repo);

    let out1 = workdir.path().join("out1");
    let out2 = workdir.path().join("out2");

    let run = |out: &PathBuf| {
        Command::cargo_bin("usta")
            .expect("binary")
            .args(["extract"])
            .arg(&repo)
            .arg("--out")
            .arg(out)
            .arg("--force")
            .assert()
            .success();
    };
    run(&out1);
    run(&out2);

    // Walk both trees and compare file sets + bytes.
    fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
            if entry.file_type().is_file() {
                let rel = entry.path().strip_prefix(root).unwrap().to_path_buf();
                let bytes = fs::read(entry.path()).unwrap();
                out.push((rel, bytes));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
    let a = snapshot_tree(&out1.join("my-existing-app-shape"));
    let b = snapshot_tree(&out2.join("my-existing-app-shape"));
    assert_eq!(a, b, "extract output should be deterministic");
}
