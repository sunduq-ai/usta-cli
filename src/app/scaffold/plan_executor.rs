//! Plan executor — applies a [`ScaffoldPlan`] against the FS + renderer
//! ports.
//!
//! Handles all three op kinds: `FileOp::Write` (with optional Jinja
//! rendering deduced from the rendered-content marker), `FileOp::Merge`
//! (JSON/TOML deep-merge), and `FileOp::Inject` (anchor-marker injection).

use std::collections::BTreeMap;

use crate::core::inject::apply_injections;
use crate::core::loaded::LoadedTemplate;
use crate::core::merge::{canonicalize_keys, deep_merge};
use crate::core::plan::{FileOp, MergeFormat, ScaffoldPlan};
use crate::core::snapshot::ManagedLock;
use crate::ports::fs::FileSystem;
use crate::ports::renderer::TemplateRenderer;
use sha2::{Digest, Sha256};

use super::ScaffoldError;

/// Execute `plan`, rendering files when the corresponding `LoadedTemplate`
/// entry was a `Render` (we re-use the loaded template here as the source
/// of truth for which paths are templated; the plan only carries bytes).
///
/// Returns a [`ManagedLock`] mapping each written path to the SHA-256 of
/// the bytes actually written. The caller (the [`ScaffoldService`])
/// persists this alongside the snapshot.
///
/// [`ScaffoldService`]: super::ScaffoldService
pub fn execute_plan<F, R>(
    plan: &ScaffoldPlan,
    template: &LoadedTemplate,
    answers: &BTreeMap<String, serde_json::Value>,
    fs: &F,
    renderer: &R,
    force: bool,
) -> Result<ManagedLock, ScaffoldError>
where
    F: FileSystem,
    R: TemplateRenderer,
{
    // Build a quick lookup: dest path → was-it-rendered?
    let mut render_paths = std::collections::BTreeSet::new();
    for f in &template.base_files {
        if f.content.is_rendered() {
            render_paths.insert(f.rel_path.clone());
        }
    }
    for files in template.feature_files.values() {
        for f in files {
            if f.content.is_rendered() {
                render_paths.insert(f.rel_path.clone());
            }
        }
    }

    let mut lock = ManagedLock::default();

    for op in &plan.ops {
        match op {
            FileOp::Write { path, contents } => {
                // Paths in `FileOp::Write` are relative to `plan.root`. The
                // FileSystem adapter is responsible for anchoring them
                // (the local-fs adapter is jailed at `plan.root`).
                let bytes_to_write: Vec<u8> = if render_paths.contains(path) {
                    let source = std::str::from_utf8(contents).map_err(|e| {
                        ScaffoldError::Render(format!("{}: not valid UTF-8: {e}", path.display()))
                    })?;
                    renderer.render(source, answers)?.into_bytes()
                } else {
                    contents.clone()
                };
                fs.write(path, &bytes_to_write, force)?;
                lock.files.insert(path.clone(), sha256_hex(&bytes_to_write));
            }
            FileOp::Merge {
                path,
                format,
                value,
            } => {
                let merged_bytes = apply_merge(fs, path, *format, value)?;
                fs.write(path, &merged_bytes, true)?;
                lock.files.insert(path.clone(), sha256_hex(&merged_bytes));
            }
            FileOp::Inject {
                path,
                contributions,
            } => {
                // Render each contribution's content through the template
                // engine so injections can use the answer context (e.g.
                // `{{ scope }}`). Unrendered text passes through unchanged.
                let rendered_contributions: Vec<crate::core::plan::AnchorContribution> =
                    contributions
                        .iter()
                        .map(|c| {
                            let rendered = renderer
                                .render(&c.content, answers)
                                .unwrap_or_else(|_| c.content.clone());
                            crate::core::plan::AnchorContribution {
                                marker: c.marker.clone(),
                                content: rendered,
                            }
                        })
                        .collect();
                let existing = fs.read(path).map_err(|e| {
                    ScaffoldError::Render(format!(
                        "inject target `{}` not present (was it produced by `Write`?): {e}",
                        path.display()
                    ))
                })?;
                let source = std::str::from_utf8(&existing).map_err(|e| {
                    ScaffoldError::Render(format!(
                        "inject target `{}` not UTF-8: {e}",
                        path.display()
                    ))
                })?;
                let injected = apply_injections(source, &rendered_contributions);
                let bytes = injected.into_bytes();
                fs.write(path, &bytes, true)?;
                lock.files.insert(path.clone(), sha256_hex(&bytes));
            }
        }
    }

    // Finalization: strip any anchor markers that no selected feature
    // injected into. Without this, a marker like `# usta:imports` in a base
    // file survives into the generated project as a stray comment whenever
    // the optional feature that targets it isn't selected. See
    // `strip_residual_markers`.
    let written: Vec<std::path::PathBuf> = lock.files.keys().cloned().collect();
    strip_residual_markers(fs, &written, &mut lock)?;

    Ok(lock)
}

/// Re-read each given managed text file and drop any leftover `usta:*`
/// marker lines, updating the lock hash for files that changed. Binary
/// files and files with no marker are left untouched (the strip is a cheap
/// no-op on already-clean content).
///
/// Callers pass only the files they wrote this run — never user-owned
/// conflict files — so a stray edit of someone's source is impossible.
pub(crate) fn strip_residual_markers<F: FileSystem>(
    fs: &F,
    paths: &[std::path::PathBuf],
    lock: &mut ManagedLock,
) -> Result<(), ScaffoldError> {
    for path in paths {
        let bytes = match fs.read(path) {
            Ok(b) => b,
            Err(_) => continue, // not on disk (e.g. conflict-only) — skip
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue; // binary file — no markers possible
        };
        if !text.contains("usta:") {
            continue;
        }
        let stripped = crate::core::inject::strip_markers(text);
        if stripped.as_bytes() != bytes.as_slice() {
            let new_bytes = stripped.into_bytes();
            fs.write(path, &new_bytes, true)?;
            lock.files.insert(path.clone(), sha256_hex(&new_bytes));
        }
    }
    Ok(())
}

/// Read existing JSON/TOML at `path` (or start empty), deep-merge `overlay`,
/// canonicalize key order, and serialize back as the original `format`.
fn apply_merge<F: FileSystem>(
    fs: &F,
    path: &std::path::Path,
    format: MergeFormat,
    overlay: &serde_json::Value,
) -> Result<Vec<u8>, ScaffoldError> {
    // Start from the file on disk if it exists, else from an empty object.
    let mut current: serde_json::Value = if fs.exists(path) {
        let bytes = fs.read(path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| {
            ScaffoldError::Render(format!("merge target `{}` not UTF-8: {e}", path.display()))
        })?;
        match format {
            MergeFormat::Json => serde_json::from_str(text).map_err(|e| {
                ScaffoldError::Render(format!("parse JSON `{}`: {e}", path.display()))
            })?,
            MergeFormat::Toml => toml::from_str(text).map_err(|e| {
                ScaffoldError::Render(format!("parse TOML `{}`: {e}", path.display()))
            })?,
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    deep_merge(&mut current, overlay);
    canonicalize_keys(&mut current);

    let out = match format {
        MergeFormat::Json => {
            let mut s = serde_json::to_string_pretty(&current)
                .map_err(|e| ScaffoldError::Render(format!("emit JSON: {e}")))?;
            s.push('\n');
            s
        }
        MergeFormat::Toml => {
            // Convert back to TOML by going through `toml::Value`.
            let toml_value: toml::Value = serde_json::from_value(current.clone())
                .map_err(|e| ScaffoldError::Render(format!("JSON→TOML convert: {e}")))?;
            toml::to_string_pretty(&toml_value)
                .map_err(|e| ScaffoldError::Render(format!("emit TOML: {e}")))?
        }
    };
    Ok(out.into_bytes())
}

/// SHA-256 hex digest of `bytes`. Stable, lowercase.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::core::loaded::{TemplateContent, TemplateFile};
    use crate::core::plan::FileOp;
    use crate::core::template::{Template, TemplateId, TemplateMeta};
    use crate::ports::fs::{FileSystem, FsError};
    use crate::ports::renderer::{RenderError, TemplateRenderer};
    use semver::Version;

    use super::*;

    /// Trivial in-memory FS for app-layer tests. App-layer tests cannot
    /// import `crate::adapters`, so we re-define a tiny local one here.
    #[derive(Default)]
    struct LocalInMem {
        files: std::sync::Mutex<BTreeMap<PathBuf, Vec<u8>>>,
    }
    impl FileSystem for LocalInMem {
        fn write(&self, path: &Path, bytes: &[u8], _force: bool) -> Result<(), FsError> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), bytes.to_vec());
            Ok(())
        }
        fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
            // The scaffold executor's marker-stripping finalization reads
            // back each written file, so this must return stored content.
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| FsError::Io {
                    path: path.to_path_buf(),
                    source: std::io::Error::from(std::io::ErrorKind::NotFound),
                })
        }
        fn exists(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }
        fn mkdir_p(&self, _: &Path) -> Result<(), FsError> {
            Ok(())
        }
        fn remove(&self, _: &Path) -> Result<(), FsError> {
            Ok(())
        }
    }

    /// Trivial renderer that replaces `{{name}}` literally.
    struct DumbRenderer;
    impl TemplateRenderer for DumbRenderer {
        fn render(
            &self,
            source: &str,
            ctx: &BTreeMap<String, serde_json::Value>,
        ) -> Result<String, RenderError> {
            let name = ctx
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(source.replace("{{name}}", &name))
        }
    }

    #[test]
    fn writes_verbatim_and_rendered_files() {
        let loaded = LoadedTemplate {
            manifest: Template {
                meta: TemplateMeta {
                    id: TemplateId("t".into()),
                    display_name: "T".into(),
                    version: Version::new(0, 1, 0),
                    min_usta: ">=0.1.0".parse().unwrap(),
                    stacks: vec![],
                },
                features: vec![],
                prompts: vec![],
            },
            base_files: vec![
                TemplateFile {
                    rel_path: PathBuf::from("README.md"),
                    content: TemplateContent::Render("# {{name}}".into()),
                },
                TemplateFile {
                    rel_path: PathBuf::from("static.txt"),
                    content: TemplateContent::Verbatim(b"raw".to_vec()),
                },
            ],
            feature_files: BTreeMap::new(),
            feature_merges: BTreeMap::new(),
            feature_injections: BTreeMap::new(),
        };

        let plan = ScaffoldPlan {
            root: PathBuf::from("/out"),
            ops: vec![
                FileOp::Write {
                    path: PathBuf::from("README.md"),
                    contents: b"# {{name}}".to_vec(),
                },
                FileOp::Write {
                    path: PathBuf::from("static.txt"),
                    contents: b"raw".to_vec(),
                },
            ],
        };

        let mut answers = BTreeMap::new();
        answers.insert("name".into(), serde_json::json!("usta"));

        let fs = LocalInMem::default();
        let r = DumbRenderer;
        let lock = execute_plan(&plan, &loaded, &answers, &fs, &r, true).unwrap();

        // Paths are relative; `plan.root` is the FS adapter's responsibility.
        let files = fs.files.lock().unwrap();
        assert_eq!(files.get(&PathBuf::from("README.md")).unwrap(), b"# usta");
        assert_eq!(files.get(&PathBuf::from("static.txt")).unwrap(), b"raw");

        // Lock entries: hash matches what was actually written.
        assert_eq!(lock.files.len(), 2);
        let readme_hash = lock.files.get(&PathBuf::from("README.md")).unwrap();
        assert_eq!(readme_hash, &sha256_hex(b"# usta"));
    }
}
