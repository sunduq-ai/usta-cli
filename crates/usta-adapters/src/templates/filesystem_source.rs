//! Filesystem-backed template source.
//!
//! Layout it expects:
//!
//! ```text
//! <root>/
//! └── <template-id>/
//!     ├── template.toml          # required
//!     ├── base/                  # optional — files copied for every scaffold
//!     │   └── …
//!     └── features/
//!         └── <feature-id>/
//!             └── files/         # files added when this feature is selected
//!                 └── …
//! ```
//!
//! Files ending in `.j2` are stripped of that extension at load time and
//! marked as [`TemplateContent::Render`]; everything else is loaded
//! verbatim.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use usta_core::loaded::{InjectionFile, LoadedTemplate, MergeFile, TemplateContent, TemplateFile};
use usta_core::paths::to_forward_slashes;
use usta_core::plan::{AnchorContribution, MergeFormat};
use usta_core::template::{Template, TemplateId};
use usta_ports::template_source::{TemplateSource, TemplateSourceError};
use walkdir::WalkDir;

/// A template source backed by a directory on disk.
#[derive(Debug, Clone)]
pub struct FilesystemTemplateSource {
    root: PathBuf,
}

impl FilesystemTemplateSource {
    /// Create a new source rooted at `root`. Each subdirectory of `root`
    /// containing a `template.toml` is one template.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn template_dir(&self, id: &TemplateId) -> PathBuf {
        self.root.join(&id.0)
    }

    fn manifest_path(&self, id: &TemplateId) -> PathBuf {
        self.template_dir(id).join("template.toml")
    }

    fn load_files_from(
        dir: &Path,
        template_id: &TemplateId,
    ) -> Result<Vec<TemplateFile>, TemplateSourceError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in WalkDir::new(dir).follow_links(false) {
            let entry = entry.map_err(|e| TemplateSourceError::Io {
                id: template_id.0.clone(),
                message: format!("walk: {e}"),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs = entry.path();
            let rel = abs.strip_prefix(dir).expect("walked under dir");

            // Strip a trailing `.j2` to derive the destination path. The
            // path is then normalized to forward slashes so downstream code
            // (snapshot, lock file, anchor markers) sees one canonical form
            // on every OS.
            let (dest_rel, render) = match rel.extension() {
                Some(ext) if ext == "j2" => {
                    let stem = rel.with_extension("");
                    (to_forward_slashes(&stem), true)
                }
                _ => (to_forward_slashes(rel), false),
            };

            let bytes = std::fs::read(abs).map_err(|e| TemplateSourceError::Io {
                id: template_id.0.clone(),
                message: format!("read {}: {e}", abs.display()),
            })?;

            let content = if render {
                let s = String::from_utf8(bytes).map_err(|_| TemplateSourceError::Io {
                    id: template_id.0.clone(),
                    message: format!("`{}.j2` is not valid UTF-8", dest_rel.display()),
                })?;
                TemplateContent::Render(s)
            } else {
                TemplateContent::Verbatim(bytes)
            };

            out.push(TemplateFile {
                rel_path: dest_rel,
                content,
            });
        }
        // Stable order by destination path.
        out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        Ok(out)
    }
}

impl TemplateSource for FilesystemTemplateSource {
    fn list_ids(&self) -> Vec<TemplateId> {
        let mut ids = Vec::new();
        let read_dir = match std::fs::read_dir(&self.root) {
            Ok(rd) => rd,
            Err(_) => return ids,
        };
        for entry in read_dir.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("template.toml").is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    ids.push(TemplateId(name.to_string()));
                }
            }
        }
        ids.sort();
        ids
    }

    fn load(&self, id: &TemplateId) -> Result<LoadedTemplate, TemplateSourceError> {
        let manifest_path = self.manifest_path(id);
        if !manifest_path.is_file() {
            return Err(TemplateSourceError::NotFound(id.0.clone()));
        }

        let manifest_text =
            std::fs::read_to_string(&manifest_path).map_err(|e| TemplateSourceError::Io {
                id: id.0.clone(),
                message: format!("read manifest: {e}"),
            })?;

        let manifest: Template =
            toml::from_str(&manifest_text).map_err(|e| TemplateSourceError::InvalidManifest {
                id: id.0.clone(),
                message: e.to_string(),
            })?;

        // Sanity: manifest's declared id matches the directory name.
        if manifest.id() != id {
            return Err(TemplateSourceError::InvalidManifest {
                id: id.0.clone(),
                message: format!("directory `{}` declares id `{}`", id.0, manifest.id().0),
            });
        }

        let template_dir = self.template_dir(id);
        let base_files = Self::load_files_from(&template_dir.join("base"), id)?;

        let mut feature_files = std::collections::BTreeMap::new();
        let mut feature_merges = std::collections::BTreeMap::new();
        let mut feature_injections = std::collections::BTreeMap::new();
        for feature in &manifest.features {
            let feature_root = template_dir.join("features").join(&feature.id.0);

            let files = Self::load_files_from(&feature_root.join("files"), id)?;
            feature_files.insert(feature.id.clone(), files);

            let merges = Self::load_merges_from(&feature_root.join("merges"), id)?;
            feature_merges.insert(feature.id.clone(), merges);

            let injections = Self::load_injections_from(&feature_root.join("injections"), id)?;
            feature_injections.insert(feature.id.clone(), injections);
        }

        Ok(LoadedTemplate {
            manifest,
            base_files,
            feature_files,
            feature_merges,
            feature_injections,
        })
    }
}

/// On-disk shape for `injections/<target>.inject.toml`.
#[derive(Debug, Deserialize)]
struct InjectionToml {
    #[serde(default, rename = "at")]
    contributions: Vec<AnchorContributionToml>,
}

#[derive(Debug, Deserialize)]
struct AnchorContributionToml {
    marker: String,
    content: String,
}

impl FilesystemTemplateSource {
    fn load_merges_from(
        dir: &Path,
        template_id: &TemplateId,
    ) -> Result<Vec<MergeFile>, TemplateSourceError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in WalkDir::new(dir).follow_links(false) {
            let entry = entry.map_err(|e| TemplateSourceError::Io {
                id: template_id.0.clone(),
                message: format!("walk merges: {e}"),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs = entry.path();
            let rel = abs.strip_prefix(dir).expect("under merges dir");

            let (target, format) = match strip_merge_suffix(rel) {
                Some(pair) => pair,
                None => continue, // ignore unknown extensions
            };

            let text = std::fs::read_to_string(abs).map_err(|e| TemplateSourceError::Io {
                id: template_id.0.clone(),
                message: format!("read merge {}: {e}", abs.display()),
            })?;
            let value: serde_json::Value = match format {
                MergeFormat::Json => serde_json::from_str(&text).map_err(|e| {
                    TemplateSourceError::InvalidManifest {
                        id: template_id.0.clone(),
                        message: format!("invalid JSON merge `{}`: {e}", abs.display()),
                    }
                })?,
                MergeFormat::Toml => {
                    toml::from_str(&text).map_err(|e| TemplateSourceError::InvalidManifest {
                        id: template_id.0.clone(),
                        message: format!("invalid TOML merge `{}`: {e}", abs.display()),
                    })?
                }
            };

            out.push(MergeFile {
                target,
                format,
                value,
            });
        }
        out.sort_by(|a, b| a.target.cmp(&b.target));
        Ok(out)
    }

    fn load_injections_from(
        dir: &Path,
        template_id: &TemplateId,
    ) -> Result<Vec<InjectionFile>, TemplateSourceError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in WalkDir::new(dir).follow_links(false) {
            let entry = entry.map_err(|e| TemplateSourceError::Io {
                id: template_id.0.clone(),
                message: format!("walk injections: {e}"),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs = entry.path();
            let rel = abs.strip_prefix(dir).expect("under injections dir");

            let target = match strip_inject_suffix(rel) {
                Some(t) => t,
                None => continue,
            };

            let text = std::fs::read_to_string(abs).map_err(|e| TemplateSourceError::Io {
                id: template_id.0.clone(),
                message: format!("read injection {}: {e}", abs.display()),
            })?;
            let parsed: InjectionToml =
                toml::from_str(&text).map_err(|e| TemplateSourceError::InvalidManifest {
                    id: template_id.0.clone(),
                    message: format!("invalid injection `{}`: {e}", abs.display()),
                })?;
            let contributions = parsed
                .contributions
                .into_iter()
                .map(|c| AnchorContribution {
                    marker: c.marker,
                    content: c.content,
                })
                .collect();

            out.push(InjectionFile {
                target,
                contributions,
            });
        }
        out.sort_by(|a, b| a.target.cmp(&b.target));
        Ok(out)
    }
}

/// `package.json.merge.json` → (`package.json`, Json).
/// `apps/api/pyproject.toml.merge.toml` → (`apps/api/pyproject.toml`, Toml).
///
/// The returned path uses forward slashes regardless of host OS — it ends
/// up in [`MergeFile::target`] which is compared against template-file
/// `rel_path`s (also normalized) and embedded in `.usta/snapshot.toml`.
fn strip_merge_suffix(rel: &Path) -> Option<(PathBuf, MergeFormat)> {
    // Normalize first so suffix matching works against `/`-joined input
    // on every OS (Windows would otherwise see `apps\api\pyproject...`).
    let normalized = to_forward_slashes(rel);
    let s = normalized.to_string_lossy();
    if let Some(stem) = s.strip_suffix(".merge.json") {
        return Some((PathBuf::from(stem), MergeFormat::Json));
    }
    s.strip_suffix(".merge.toml")
        .map(|stem| (PathBuf::from(stem), MergeFormat::Toml))
}

/// `apps/api/main.py.inject.toml` → `apps/api/main.py`.
///
/// Forward-slash normalized for the same reason as [`strip_merge_suffix`].
fn strip_inject_suffix(rel: &Path) -> Option<PathBuf> {
    let normalized = to_forward_slashes(rel);
    let s = normalized.to_string_lossy();
    s.strip_suffix(".inject.toml").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn write(p: &Path, contents: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, contents).unwrap();
    }

    fn fixture(root: &Path) {
        // templates/
        // └── demo/
        //     ├── template.toml
        //     ├── base/
        //     │   ├── README.md.j2
        //     │   └── static/data.bin
        //     └── features/
        //         └── extra/
        //             └── files/
        //                 └── extra.txt
        write(
            &root.join("demo/template.toml"),
            r#"
[template]
id           = "demo"
display_name = "Demo"
version      = "0.1.0"
min_usta     = ">=0.1.0"
stacks       = []

[[features]]
id           = "extra"
display_name = "Extra"
default      = true
requires     = []
conflicts    = []
stacks       = []
"#,
        );
        write(
            &root.join("demo/base/README.md.j2"),
            "# {{ project_name }}\n",
        );
        // Binary-ish file: not UTF-8 strict, must NOT trip the .j2 path.
        fs::create_dir_all(root.join("demo/base/static")).unwrap();
        fs::write(root.join("demo/base/static/data.bin"), [0u8, 1, 2, 3]).unwrap();
        write(
            &root.join("demo/features/extra/files/extra.txt"),
            "hello extra\n",
        );
    }

    #[test]
    fn list_ids_finds_demo() {
        let d = tempdir().unwrap();
        fixture(d.path());
        let src = FilesystemTemplateSource::new(d.path());
        assert_eq!(src.list_ids(), vec![TemplateId("demo".into())]);
    }

    #[test]
    fn loads_manifest_and_files() {
        let d = tempdir().unwrap();
        fixture(d.path());
        let src = FilesystemTemplateSource::new(d.path());
        let loaded = src.load(&TemplateId("demo".into())).unwrap();

        assert_eq!(loaded.manifest.id().0, "demo");
        assert_eq!(loaded.manifest.features.len(), 1);

        // base
        let base_paths: Vec<_> = loaded
            .base_files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert!(base_paths.contains(&"README.md".into()));
        assert!(base_paths.contains(&"static/data.bin".into()));

        // README is .j2 → Render; data.bin is verbatim
        let readme = loaded
            .base_files
            .iter()
            .find(|f| f.rel_path.display().to_string() == "README.md")
            .unwrap();
        assert!(readme.content.is_rendered());

        let bin = loaded
            .base_files
            .iter()
            .find(|f| f.rel_path.display().to_string() == "static/data.bin")
            .unwrap();
        assert!(!bin.content.is_rendered());

        // feature files
        let extra_files = loaded
            .feature_files
            .get(&usta_core::template::FeatureId("extra".into()))
            .unwrap();
        assert_eq!(extra_files.len(), 1);
        assert_eq!(extra_files[0].rel_path.display().to_string(), "extra.txt");
    }

    #[test]
    fn load_unknown_id_errors() {
        let d = tempdir().unwrap();
        fixture(d.path());
        let src = FilesystemTemplateSource::new(d.path());
        let err = src.load(&TemplateId("nope".into())).unwrap_err();
        assert!(matches!(err, TemplateSourceError::NotFound(_)));
    }

    #[test]
    fn directory_id_must_match_manifest_id() {
        let d = tempdir().unwrap();
        // Manifest says `demo`, but we'll put it under `wrong/`.
        write(
            &d.path().join("wrong/template.toml"),
            r#"
[template]
id           = "demo"
display_name = "Demo"
version      = "0.1.0"
min_usta     = ">=0.1.0"
"#,
        );
        let src = FilesystemTemplateSource::new(d.path());
        let err = src.load(&TemplateId("wrong".into())).unwrap_err();
        assert!(matches!(err, TemplateSourceError::InvalidManifest { .. }));
    }
}
