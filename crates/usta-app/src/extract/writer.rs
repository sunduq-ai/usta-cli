//! Writer for [`ExtractedTemplate`] — serializes manifest + files to disk
//! via the [`FileSystem`] port.

use std::path::{Path, PathBuf};

use usta_core::paths::to_forward_slashes;
use usta_ports::fs::FileSystem;

use super::{ExtractError, ExtractedTemplate};

/// Write the synthesized template under `out_root/<template_id>/` using
/// `fs`. The FS adapter is responsible for the write-jail (callers should
/// pass a real-filesystem adapter rooted at `out_root` or a parent).
pub fn write_extracted_template<F: FileSystem>(
    fs: &F,
    template: &ExtractedTemplate,
    force: bool,
) -> Result<usize, ExtractError> {
    let template_dir: PathBuf = PathBuf::from(&template.manifest.id().0);

    // Manifest first. Normalize the destination so paths handed to the FS
    // adapter (and any in-memory map) are portable across Windows/macOS/Linux.
    let manifest_text = toml::to_string_pretty(&template.manifest)
        .map_err(|e| ExtractError::Synthesis(format!("serialize manifest: {e}")))?;
    fs.write(
        &to_forward_slashes(&template_dir.join("template.toml")),
        manifest_text.as_bytes(),
        force,
    )?;

    // Files.
    for file in &template.files {
        let dest = to_forward_slashes(&template_dir.join(&file.rel_path));
        fs.write(&dest, &file.bytes, force)?;
    }

    Ok(template.files.len() + 1)
}

/// Helper: convert a project-root-relative path to its corresponding
/// destination under the template tree's `base/` directory.
///
/// Tests live next to this so the path mechanics stay verifiable without a
/// full extract run. The returned path always uses forward slashes so
/// downstream code (snapshot, lock file) stays cross-platform stable.
pub fn base_path(rel: &Path) -> PathBuf {
    to_forward_slashes(&PathBuf::from("base").join(rel))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use semver::{Version, VersionReq};
    use usta_core::template::{Template, TemplateId, TemplateMeta};
    use usta_ports::fs::{FileSystem, FsError};

    use super::*;

    /// Tiny in-memory FS local to this module. We keep it here because the
    /// app crate isn't allowed to import `usta-adapters`, but we want a
    /// concrete `FileSystem` to verify writes.
    #[derive(Default)]
    struct Mem {
        files: std::sync::Mutex<BTreeMap<PathBuf, Vec<u8>>>,
    }
    impl FileSystem for Mem {
        fn write(&self, path: &Path, bytes: &[u8], _force: bool) -> Result<(), FsError> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), bytes.to_vec());
            Ok(())
        }
        fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
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

    fn template_with_id(id: &str) -> Template {
        Template {
            meta: TemplateMeta {
                id: TemplateId(id.into()),
                display_name: id.into(),
                version: Version::new(0, 1, 0),
                min_usta: VersionReq::parse(">=0.1.0").unwrap(),
                stacks: vec![],
            },
            features: vec![],
            prompts: vec![],
        }
    }

    #[test]
    fn writes_manifest_and_files_under_template_dir() {
        let extracted = ExtractedTemplate {
            manifest: template_with_id("demo"),
            files: vec![
                super::super::TemplateOutFile {
                    rel_path: PathBuf::from("base/README.md"),
                    bytes: b"hi".to_vec(),
                    is_text: true,
                },
                super::super::TemplateOutFile {
                    rel_path: PathBuf::from("base/static/data.bin"),
                    bytes: vec![0u8, 1, 2],
                    is_text: false,
                },
            ],
            dropped: 0,
        };

        let fs = Mem::default();
        let written = write_extracted_template(&fs, &extracted, true).unwrap();
        assert_eq!(written, 3); // 2 files + 1 manifest

        let files = fs.files.lock().unwrap();
        assert!(files.contains_key(&PathBuf::from("demo/template.toml")));
        assert!(files.contains_key(&PathBuf::from("demo/base/README.md")));
        assert!(files.contains_key(&PathBuf::from("demo/base/static/data.bin")));

        // Manifest serializes to TOML and round-trips.
        let manifest_bytes = files.get(&PathBuf::from("demo/template.toml")).unwrap();
        let manifest_text = std::str::from_utf8(manifest_bytes).unwrap();
        let parsed: Template = toml::from_str(manifest_text).unwrap();
        assert_eq!(parsed.id().0, "demo");
    }

    #[test]
    fn base_path_helper() {
        assert_eq!(
            base_path(Path::new("apps/api/main.py"))
                .display()
                .to_string(),
            "base/apps/api/main.py"
        );
    }
}
