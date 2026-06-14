//! Composition root.
//!
//! This is the **only** module outside `crate::adapters` allowed to mention
//! concrete adapter types. `crate::commands::*` may also instantiate adapters
//! directly — they are part of the binary, not the use-case layer, and the
//! exemption keeps simple read-only commands (e.g. `usta list`) from needing
//! a wiring helper each. The layer rule used to be Cargo-enforced when this
//! lived in `usta-cli`; since the v0.1.0 single-crate collapse it's a code
//! review responsibility. See `docs/ARCHITECTURE.md` and
//! `docs/ADR/0002-single-crate-collapse.md`.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use include_dir::{include_dir, Dir};

use crate::adapters::clock::SystemClock;
use crate::adapters::fs::LocalFs;
use crate::adapters::prompts::inquire_ui::InquireUi;
use crate::adapters::prompts::noninteractive::NoninteractiveUi;
use crate::adapters::renderer::MinijinjaRenderer;
use crate::adapters::templates::filesystem_source::FilesystemTemplateSource;
use crate::app::scaffold::ScaffoldService;

/// The built-in templates, embedded into the binary at compile time so a
/// `cargo install`ed `usta` works out of the box without a `--templates-dir`.
static BUILTIN_TEMPLATES: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/templates");

/// Build a `ScaffoldService` wired with the local FS + minijinja renderer
/// + system clock.
pub fn build_scaffold_service(
    project_root: PathBuf,
) -> ScaffoldService<LocalFs, MinijinjaRenderer, SystemClock> {
    let fs = LocalFs::new(project_root);
    let renderer = MinijinjaRenderer::new();
    let clock = SystemClock::new();
    ScaffoldService::new(fs, renderer, clock)
}

/// Build a filesystem template source rooted at `dir`.
pub fn build_template_source(dir: PathBuf) -> FilesystemTemplateSource {
    FilesystemTemplateSource::new(dir)
}

/// Resolve the templates directory every read command (`new`, `add`,
/// `update`, `list`) should use. Precedence:
///
/// 1. `explicit` — the `--templates-dir` flag (also populated from
///    `USTA_TEMPLATES_DIR` by clap).
/// 2. A `templates/` directory found by walking up from `start` (the dev
///    workflow: running from inside the repo, or a user's own template repo).
/// 3. The built-in templates embedded in the binary, extracted to a
///    per-version cache directory on first use. This is what makes
///    `cargo install usta && usta new --template nx-monorepo` work.
///
/// Because the built-ins are reused as the fallback, the headline command
/// never dead-ends on "no templates directory found".
pub fn resolve_templates_dir(explicit: Option<&Path>, start: &Path) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.is_dir() {
            return Err(anyhow!(
                "--templates-dir is not a directory: {}",
                p.display()
            ));
        }
        return Ok(p.to_path_buf());
    }
    for dir in start.ancestors() {
        let cand = dir.join("templates");
        if cand.join("hello-world").is_dir() || cand.join("nx-monorepo").is_dir() {
            return Ok(cand);
        }
    }
    extract_builtin_templates()
}

/// Extract the embedded built-in templates to a stable per-version cache
/// directory and return its path. Idempotent: if a complete extraction for
/// this `usta` version already exists, it's reused without rewriting.
fn extract_builtin_templates() -> Result<PathBuf> {
    let version = env!("CARGO_PKG_VERSION");
    let target = cache_root().join(format!("templates-{version}"));

    // A sentinel file proves the extraction completed (guards against a
    // half-written cache from an interrupted earlier run).
    let sentinel = target.join("hello-world/template.toml");
    if sentinel.is_file() {
        return Ok(target);
    }

    // Extract into a sibling temp dir, then atomically rename into place so
    // concurrent `usta` processes never observe a partial tree.
    let tmp = cache_root().join(format!("templates-{version}.tmp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)
        .with_context(|| format!("creating template cache dir {}", tmp.display()))?;
    BUILTIN_TEMPLATES
        .extract(&tmp)
        .with_context(|| format!("extracting built-in templates to {}", tmp.display()))?;

    match std::fs::rename(&tmp, &target) {
        Ok(()) => {}
        Err(_) => {
            // Another process likely won the race, or rename across the same
            // dir failed transiently. If the target is now valid, use it;
            // otherwise surface the original error.
            let _ = std::fs::remove_dir_all(&tmp);
            if !sentinel.is_file() {
                return Err(anyhow!(
                    "failed to populate template cache at {}",
                    target.display()
                ));
            }
        }
    }
    Ok(target)
}

/// Per-user cache root for usta. XDG on Unix, `%LOCALAPPDATA%` on Windows,
/// falling back to the OS temp dir. No external `dirs` dependency.
fn cache_root() -> PathBuf {
    if let Some(x) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(x).join("usta");
    }
    if let Some(h) = std::env::var_os("HOME") {
        return PathBuf::from(h).join(".cache").join("usta");
    }
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(l).join("usta").join("cache");
    }
    std::env::temp_dir().join("usta")
}

/// Choose interactive vs. non-interactive UI based on the `--yes` flag.
/// Returned as a boxed trait object so the call site stays polymorphic.
pub fn build_prompt_ui(non_interactive: bool) -> Box<dyn crate::ports::prompts::PromptUi> {
    if non_interactive {
        Box::new(NoninteractiveUi)
    } else {
        Box::new(InquireUi::new())
    }
}
