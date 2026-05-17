//! `ExtractService` — orchestrates scan → synthesize → write.
//!
//! Generic over the scanner, FS, and (for tests) a function that reads
//! file bytes given a path. The binary wires this with the real
//! ignore-respecting scanner + local FS; tests inject in-memory
//! equivalents.

use std::path::{Path, PathBuf};

use usta_core::extract::ExtractConfig;
use usta_ports::fs::FileSystem;
use usta_ports::repo_scanner::{RepoScanner, ScannedFile};

use super::{synthesizer, writer, ExtractError, ExtractedTemplate};

/// The orchestrator. The `read_bytes` closure lets us read file contents
/// without dragging a second filesystem trait into ports — adapters will
/// just pass `|p| std::fs::read(p)` here.
pub struct ExtractService<'a, S, F, R>
where
    S: RepoScanner,
    F: FileSystem,
    R: Fn(&Path) -> std::io::Result<Vec<u8>> + 'a,
{
    scanner: S,
    out_fs: F,
    read_bytes: R,
    _life: std::marker::PhantomData<&'a ()>,
}

impl<'a, S, F, R> ExtractService<'a, S, F, R>
where
    S: RepoScanner,
    F: FileSystem,
    R: Fn(&Path) -> std::io::Result<Vec<u8>> + 'a,
{
    /// Construct.
    pub fn new(scanner: S, out_fs: F, read_bytes: R) -> Self {
        Self {
            scanner,
            out_fs,
            read_bytes,
            _life: std::marker::PhantomData,
        }
    }

    /// Run the pipeline against `repo_root` and write the synthesized
    /// template under the configured FS root.
    pub fn run(
        &self,
        repo_root: &Path,
        config: &ExtractConfig,
        force: bool,
    ) -> Result<ExtractOutcome, ExtractError> {
        // 1. Scan.
        let scanned: Vec<ScannedFile> = self
            .scanner
            .scan(repo_root)
            .map_err(|e| ExtractError::Synthesis(format!("scan: {e}")))?;
        let scanned_count = scanned.len();

        // 2. Read bytes.
        let mut pairs: Vec<(PathBuf, Vec<u8>)> = Vec::with_capacity(scanned.len());
        for s in &scanned {
            let abs = repo_root.join(&s.rel_path);
            let bytes = (self.read_bytes)(&abs)
                .map_err(|e| ExtractError::Synthesis(format!("read {}: {e}", abs.display())))?;
            pairs.push((s.rel_path.clone(), bytes));
        }

        // 3. Synthesize (pure).
        let extracted: ExtractedTemplate = synthesizer::synthesize(&pairs, config)?;

        // 4. Write.
        let written = writer::write_extracted_template(&self.out_fs, &extracted, force)?;

        Ok(ExtractOutcome {
            scanned: scanned_count,
            dropped: extracted.dropped,
            written,
            template_id: extracted.manifest.id().0.clone(),
            features: extracted
                .manifest
                .features
                .iter()
                .map(|f| f.id.0.clone())
                .collect(),
        })
    }
}

/// Result of an extract run.
#[derive(Debug, Clone)]
pub struct ExtractOutcome {
    /// Total files scanned (after `.gitignore` exclusions).
    pub scanned: usize,
    /// Files dropped by the noise filter / user `drop_paths`.
    pub dropped: usize,
    /// Files written into the synthesized template (incl. manifest).
    pub written: usize,
    /// Template id of the synthesized template.
    pub template_id: String,
    /// Synthesized feature ids.
    pub features: Vec<String>,
}
