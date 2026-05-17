//! Repository-scanning port (used by `usta extract`).

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Scan errors.
#[derive(Debug, Error)]
pub enum ScanError {
    /// I/O failure during traversal.
    #[error("scan io error: {0}")]
    Io(String),
}

/// One file discovered during a scan.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Path relative to the scan root.
    pub rel_path: PathBuf,
    /// Size in bytes (for triage; full reads happen later).
    pub size: u64,
}

/// Scans a repository, respecting ignore files.
pub trait RepoScanner {
    /// Walk `root` and return discovered files. Implementations should respect
    /// `.gitignore` and a `.usta-extract-ignore` if present.
    fn scan(&self, root: &Path) -> Result<Vec<ScannedFile>, ScanError>;
}
