//! Integration test for the `nx-monorepo` template.
//!
//! Exercises every feature category we ship in P2:
//! - base workspace files (package.json, nx.json, tsconfig.base.json, …)
//! - API features (fastapi + mongodb + auth-jwt) with anchor injection +
//!   pyproject.toml deep-merge
//! - Web features (vite-react + router + tanstack-query + i18n) with JSX
//!   anchor injection + package.json deep-merge
//! - Shared packages (types + utils + ui) with tsconfig path injection
//! - Tooling (husky) with package.json deep-merge
//! - Snapshot + lock files

use std::path::PathBuf;

use assert_cmd::Command;
use tempfile::tempdir;

fn templates_dir() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../templates")
        .canonicalize()
        .expect("templates dir")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn scaffolds_with_default_features_only() {
    let workdir = tempdir().expect("tempdir");
    let name = "default-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["new", name, "--template", "nx-monorepo", "--templates-dir"])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(name);

    // Base workspace files.
    assert!(project.join("pnpm-workspace.yaml").is_file());
    assert!(project.join("nx.json").is_file());
    assert!(project.join("tsconfig.base.json").is_file());
    assert!(project.join("eslint.config.js").is_file());
    assert!(project.join(".gitignore").is_file());
    assert!(project.join(".prettierrc").is_file());

    // AGENTS.md.j2 → AGENTS.md, rendered.
    let agents = read(&project.join("AGENTS.md"));
    assert!(agents.contains("# AGENTS.md — default-app"));

    // Default features: api-fastapi + web-vite-react + shared-types.
    assert!(project.join("apps/api/pyproject.toml").is_file());
    assert!(project.join("apps/api/src/main.py").is_file());
    assert!(project.join("apps/web/package.json").is_file());
    assert!(project.join("apps/web/src/main.tsx").is_file());
    assert!(project.join("packages/shared/types/src/index.ts").is_file());

    // Snapshot + lock.
    let snap = read(&project.join(".usta/snapshot.toml"));
    assert!(snap.contains(r#"template_id = "nx-monorepo""#));
    assert!(snap.contains("api-fastapi"));
    assert!(project.join(".usta/managed.lock").is_file());

    // ─────────── Regression: rendered files must be USABLE ───────────
    // Past bug: App.tsx.j2 used escaped Jinja delimiters
    // (`{{ '{{' }} project_name {{ '}}' }}`), which rendered to literal
    // `{{ project_name }}` inside JSX — TS treats that as object shorthand
    // for a non-existent variable and `tsc --noEmit` fails.
    let app_tsx = read(&project.join("apps/web/src/App.tsx"));
    assert!(
        app_tsx.contains(">default-app<"),
        "App.tsx must interpolate project_name as text inside the heading, got:\n{app_tsx}"
    );
    assert!(
        !app_tsx.contains("{{") && !app_tsx.contains("}}"),
        "App.tsx must not contain unescaped Jinja delimiters:\n{app_tsx}"
    );

    // Past bug: api dev tools (mypy/pytest/ruff) lived under
    // `[project.optional-dependencies]`, requiring `--extra dev` for every
    // `uv run` invocation. Migrated to PEP 735 `[dependency-groups]` so
    // `uv sync` installs them by default.
    let pyproject = read(&project.join("apps/api/pyproject.toml"));
    assert!(
        pyproject.lines().any(|l| l.trim() == "[dependency-groups]"),
        "api pyproject.toml should declare a `[dependency-groups]` section:\n{pyproject}"
    );
    assert!(
        !pyproject
            .lines()
            .any(|l| l.trim() == "[project.optional-dependencies]"),
        "api pyproject.toml should NOT have a `[project.optional-dependencies]` section header (use `[dependency-groups]` instead):\n{pyproject}"
    );
    assert!(pyproject.contains("mypy"));
    assert!(pyproject.contains("pytest"));
    assert!(pyproject.contains("ruff"));
}

#[test]
fn full_stack_with_all_api_and_web_features() {
    let workdir = tempdir().expect("tempdir");
    let name = "full-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            name,
            "--template",
            "nx-monorepo",
            "--features",
            "api-fastapi,api-mongodb,api-auth-jwt,web-vite-react,web-router,web-tanstack-query,web-i18n,shared-types,shared-utils,shared-ui,tooling-husky",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(name);

    // ─────────── API: pyproject.toml deep-merge ───────────
    // Base ships fastapi/uvicorn; api-mongodb adds motor; api-auth-jwt adds
    // python-jose, passlib, email-validator. Verify all are present.
    let pyproject = read(&project.join("apps/api/pyproject.toml"));
    assert!(pyproject.contains("fastapi"));
    assert!(pyproject.contains("motor"));
    assert!(pyproject.contains("python-jose"));
    assert!(pyproject.contains("passlib"));

    // ─────────── API: main.py anchor injections ───────────
    // mongodb injects connect/disconnect; auth-jwt injects router import +
    // include_router. The anchor markers themselves must be stripped.
    let main_py = read(&project.join("apps/api/src/main.py"));
    assert!(main_py.contains("connect_to_mongo"));
    assert!(main_py.contains("await connect_to_mongo()"));
    assert!(main_py.contains("auth_router"));
    assert!(main_py.contains(r#"app.include_router(auth_router"#));
    assert!(
        !main_py.contains("usta:imports"),
        "marker leaked:\n{main_py}"
    );
    assert!(
        !main_py.contains("usta:routers"),
        "marker leaked:\n{main_py}"
    );
    assert!(
        !main_py.contains("usta:lifespan"),
        "marker leaked:\n{main_py}"
    );

    // ─────────── api-mongodb: injection content uses project_name ───────────
    // The mongodb feature's config.py.inject.toml has
    //   MONGODB_DATABASE: str = "{{ project_name }}"
    // which only renders correctly if injection content is run through
    // the template engine (the P1.i bug fix).
    let config_py = read(&project.join("apps/api/src/infrastructure/config.py"));
    assert!(
        config_py.contains(r#"MONGODB_DATABASE: str = "full-app""#),
        "injection content not rendered:\n{config_py}"
    );
    assert!(
        !config_py.contains("usta:settings"),
        "marker leaked:\n{config_py}"
    );

    // ─────────── Web: package.json deep-merge ───────────
    // Base ships react/vite/tailwind; router adds react-router-dom; tanstack
    // adds @tanstack/react-query; i18n adds i18next.
    let pkg_text = read(&project.join("apps/web/package.json"));
    let pkg: serde_json::Value = serde_json::from_str(&pkg_text).expect("valid JSON");
    let deps = &pkg["dependencies"];
    assert!(deps.get("react").is_some());
    assert!(deps.get("react-router-dom").is_some());
    assert!(deps.get("@tanstack/react-query").is_some());
    assert!(deps.get("i18next").is_some());
    assert!(deps.get("react-i18next").is_some());

    // ─────────── Web: main.tsx JSX anchor injections ───────────
    let main_tsx = read(&project.join("apps/web/src/main.tsx"));
    assert!(main_tsx.contains("BrowserRouter"));
    assert!(main_tsx.contains("QueryClientProvider"));
    assert!(main_tsx.contains("I18nProvider"));
    assert!(
        !main_tsx.contains("usta:imports"),
        "marker leaked:\n{main_tsx}"
    );
    assert!(
        !main_tsx.contains("usta:provider-entries"),
        "marker leaked:\n{main_tsx}"
    );

    // ─────────── tsconfig.base.json path injections ───────────
    let tsconfig = read(&project.join("tsconfig.base.json"));
    assert!(tsconfig.contains("@full-app/shared-types"));
    assert!(tsconfig.contains("@full-app/shared-utils"));
    assert!(tsconfig.contains("@full-app/shared-ui"));
    assert!(
        !tsconfig.contains("usta:tsconfig-paths"),
        "marker leaked:\n{tsconfig}"
    );

    // ─────────── Husky: root package.json merge ───────────
    let root_pkg_text = read(&project.join("package.json"));
    let root_pkg: serde_json::Value = serde_json::from_str(&root_pkg_text).expect("valid JSON");
    assert_eq!(root_pkg["scripts"]["prepare"], "husky");
    assert!(root_pkg["devDependencies"].get("husky").is_some());
    assert!(root_pkg["lint-staged"].is_object());

    // ─────────── Snapshot records all features ───────────
    let snap = read(&project.join(".usta/snapshot.toml"));
    for f in [
        "api-fastapi",
        "api-mongodb",
        "api-auth-jwt",
        "web-vite-react",
        "web-router",
        "web-tanstack-query",
        "web-i18n",
        "shared-types",
        "shared-utils",
        "shared-ui",
        "tooling-husky",
    ] {
        assert!(snap.contains(f), "snapshot missing feature `{f}`");
    }
}

#[test]
fn mobile_expo_and_docker_features_scaffold_cleanly() {
    // These two features aren't covered by the full-stack test (mobile is a
    // heavy compile loop, docker references services); verify they at least
    // produce well-formed files when explicitly selected.
    let workdir = tempdir().expect("tempdir");
    let name = "extras-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            name,
            "--template",
            "nx-monorepo",
            "--features",
            "api-fastapi,web-vite-react,mobile-expo,docker",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(name);

    // Mobile expo: package.json parses; App.tsx rendered.
    let mobile_pkg = read(&project.join("apps/mobile/package.json"));
    let parsed: serde_json::Value = serde_json::from_str(&mobile_pkg).expect("mobile pkg JSON");
    assert!(parsed["dependencies"]["expo"].is_string());
    let app_tsx = read(&project.join("apps/mobile/App.tsx"));
    assert!(app_tsx.contains("extras-app"));

    // Docker: compose file rendered with project name; web Dockerfile copied.
    let compose = read(&project.join("docker-compose.yaml"));
    assert!(compose.contains("extras-app-api"));
    assert!(compose.contains("extras-app-net"));
    assert!(project.join("apps/web/Dockerfile").is_file());
}

#[test]
fn lock_file_has_one_line_per_managed_file() {
    let workdir = tempdir().expect("tempdir");
    let name = "lock-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            name,
            "--template",
            "nx-monorepo",
            "--features",
            "api-fastapi",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(name);
    let lock = read(&project.join(".usta/managed.lock"));

    // Format: header lines starting with `#`, then `<sha256>  <path>`.
    let mut header_lines = 0;
    let mut entry_lines = 0;
    for line in lock.lines() {
        if line.starts_with('#') {
            header_lines += 1;
        } else if !line.is_empty() {
            // sha256 hex digest is 64 chars, then two spaces, then path.
            let (digest, _path) = line.split_once("  ").expect("digest  path");
            assert_eq!(digest.len(), 64, "sha256 digest must be 64 hex chars");
            assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
            entry_lines += 1;
        }
    }
    assert!(header_lines >= 2, "expected lock header lines");
    assert!(
        entry_lines >= 5,
        "expected at least 5 managed files for api-fastapi default"
    );
}

#[test]
fn snapshot_file_is_valid_toml() {
    let workdir = tempdir().expect("tempdir");
    let name = "snap-app";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args(["new", name, "--template", "nx-monorepo", "--templates-dir"])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(name);
    let snap_text = read(&project.join(".usta/snapshot.toml"));
    let parsed: toml::Value = toml::from_str(&snap_text).expect("valid TOML snapshot");
    assert_eq!(
        parsed["template_id"].as_str(),
        Some("nx-monorepo"),
        "template_id"
    );
    assert!(parsed["template_version"].is_str());
    assert!(parsed["created_at"].is_str());
    assert!(parsed["features"].is_array());
}

#[test]
fn requires_are_auto_included() {
    // Selecting only `api-mongodb` should auto-pull `api-fastapi` (its
    // declared `requires`). The resolver tests already cover this in
    // isolation; here we verify the binary surface honors it.
    let workdir = tempdir().expect("tempdir");
    let name = "auto-deps";

    Command::cargo_bin("usta")
        .expect("binary")
        .current_dir(workdir.path())
        .args([
            "new",
            name,
            "--template",
            "nx-monorepo",
            "--features",
            "api-mongodb",
            "--templates-dir",
        ])
        .arg(templates_dir())
        .arg("--yes")
        .arg("--no-git")
        .arg("--no-install")
        .assert()
        .success();

    let project = workdir.path().join(name);
    assert!(project.join("apps/api/src/main.py").is_file());
    let snap = read(&project.join(".usta/snapshot.toml"));
    assert!(snap.contains("api-fastapi"));
    assert!(snap.contains("api-mongodb"));
}
