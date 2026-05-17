//! Filesystem port.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Adapter errors surfaced by [`FileSystem`] implementations.
#[derive(Debug, Error)]
pub enum FsError {
    /// Path resolves outside the configured write jail (path traversal).
    #[error("path escapes write jail: {0}")]
    PathEscape(PathBuf),
    /// Destination already exists and overwriting was not requested.
    #[error("path already exists: {0}")]
    AlreadyExists(PathBuf),
    /// Underlying I/O failure.
    #[error("io error at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Filesystem operations needed by the scaffold engine.
///
/// Implementations MUST refuse to write outside the configured write jail.
/// A `proptest` property test in `crate::adapters` enforces this for the local
/// implementation.
pub trait FileSystem {
    /// Write `bytes` to `path`. Creates parent directories. Fails if the path
    /// already exists unless `force` is true.
    fn write(&self, path: &Path, bytes: &[u8], force: bool) -> Result<(), FsError>;

    /// Read the entire contents of `path`.
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError>;

    /// Whether `path` exists.
    fn exists(&self, path: &Path) -> bool;

    /// Create directory and all parents.
    fn mkdir_p(&self, path: &Path) -> Result<(), FsError>;

    /// Remove file or directory tree at `path`.
    fn remove(&self, path: &Path) -> Result<(), FsError>;
}
