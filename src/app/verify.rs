//! `usta verify` use case — detect drift between `.usta/managed.lock` and
//! the project's actual file contents.
//!
//! Three drift categories:
//! - **Modified**: a managed file's current SHA-256 differs from the lock.
//! - **Missing**: a managed file no longer exists on disk.
//! - **Unchanged**: still matches the lock.
//!
//! Pure modulo the FS port. The CLI maps non-empty drift to exit code 41
//! (per `docs/ARCHITECTURE.md`).

use std::path::PathBuf;

use crate::core::snapshot::{ManagedLock, ManagedLockParseError};
use crate::ports::fs::{FileSystem, FsError};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::scaffold::snapshot::LOCK_PATH;

/// Where the lock and snapshot files live, mirrored from `scaffold::snapshot`.
pub use super::scaffold::snapshot::SNAPSHOT_PATH;

/// Errors raised by [`verify`].
#[derive(Debug, Error)]
pub enum VerifyError {
    /// The `.usta/managed.lock` file is missing — the project wasn't
    /// scaffolded by usta, or its `.usta/` was deleted.
    #[error("no `.usta/managed.lock` found at project root (was this scaffolded by usta?)")]
    NoLockFile,
    /// Lock file present but malformed.
    #[error("invalid managed.lock: {0}")]
    InvalidLock(#[from] ManagedLockParseError),
    /// FS port returned an error.
    #[error("filesystem: {0}")]
    Fs(String),
}

impl From<FsError> for VerifyError {
    fn from(e: FsError) -> Self {
        VerifyError::Fs(e.to_string())
    }
}

/// Result of a verify run.
#[derive(Debug, Clone, Default)]
pub struct VerifyReport {
    /// Files whose hash matches the lock.
    pub unchanged: Vec<PathBuf>,
    /// Files whose hash differs from the lock (user edited).
    pub modified: Vec<PathBuf>,
    /// Files listed in the lock but absent on disk.
    pub missing: Vec<PathBuf>,
}

impl VerifyReport {
    /// Whether any drift was detected.
    pub fn is_clean(&self) -> bool {
        self.modified.is_empty() && self.missing.is_empty()
    }

    /// Total managed files inspected.
    pub fn total(&self) -> usize {
        self.unchanged.len() + self.modified.len() + self.missing.len()
    }
}

/// Read `.usta/managed.lock` and compare each entry's hash against disk.
///
/// `fs` should be jailed at the project root.
pub fn verify<F: FileSystem>(fs: &F) -> Result<VerifyReport, VerifyError> {
    let lock_path = std::path::Path::new(LOCK_PATH);
    if !fs.exists(lock_path) {
        return Err(VerifyError::NoLockFile);
    }
    let lock_bytes = fs.read(lock_path)?;
    let lock_text = std::str::from_utf8(&lock_bytes)
        .map_err(|e| VerifyError::Fs(format!("managed.lock not UTF-8: {e}")))?;
    let lock = ManagedLock::from_text(lock_text)?;

    let mut report = VerifyReport::default();
    for (path, expected_digest) in &lock.files {
        if !fs.exists(path) {
            report.missing.push(path.clone());
            continue;
        }
        let bytes = fs.read(path)?;
        let actual = sha256_hex(&bytes);
        if &actual == expected_digest {
            report.unchanged.push(path.clone());
        } else {
            report.modified.push(path.clone());
        }
    }
    Ok(report)
}

/// SHA-256 hex digest. Stable, lowercase. Mirrors `scaffold::plan_executor::sha256_hex`
/// — kept as a separate copy here so this module stands on its own.
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
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Mutex;

    use crate::ports::fs::FileSystem;

    use super::*;

    /// In-memory FS for app-layer tests (we can't import `crate::adapters` here).
    #[derive(Default)]
    struct Mem {
        files: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
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

    fn seed_lock_and_files(fs: &Mem, files: &[(&str, &[u8])]) {
        let mut lock = ManagedLock::default();
        for (path, bytes) in files {
            lock.files.insert(PathBuf::from(path), sha256_hex(bytes));
            fs.write(Path::new(path), bytes, true).unwrap();
        }
        fs.write(Path::new(LOCK_PATH), lock.to_text().as_bytes(), true)
            .unwrap();
    }

    #[test]
    fn unchanged_when_files_match_lock() {
        let fs = Mem::default();
        seed_lock_and_files(&fs, &[("README.md", b"hi"), ("a/b.txt", b"x")]);
        let report = verify(&fs).unwrap();
        assert!(report.is_clean());
        assert_eq!(report.unchanged.len(), 2);
        assert!(report.modified.is_empty());
        assert!(report.missing.is_empty());
    }

    #[test]
    fn detects_modified_file() {
        let fs = Mem::default();
        seed_lock_and_files(&fs, &[("README.md", b"hi")]);
        // User edits.
        fs.write(Path::new("README.md"), b"changed", true).unwrap();

        let report = verify(&fs).unwrap();
        assert!(!report.is_clean());
        assert_eq!(report.modified, vec![PathBuf::from("README.md")]);
    }

    #[test]
    fn detects_missing_file() {
        let fs = Mem::default();
        seed_lock_and_files(&fs, &[("README.md", b"hi"), ("a/b.txt", b"x")]);
        // User deletes one.
        fs.files.lock().unwrap().remove(&PathBuf::from("a/b.txt"));

        let report = verify(&fs).unwrap();
        assert_eq!(report.missing, vec![PathBuf::from("a/b.txt")]);
        assert_eq!(report.unchanged.len(), 1);
    }

    #[test]
    fn errors_when_lock_missing() {
        let fs = Mem::default();
        let err = verify(&fs).unwrap_err();
        assert!(matches!(err, VerifyError::NoLockFile));
    }

    #[test]
    fn errors_on_malformed_lock() {
        let fs = Mem::default();
        fs.write(Path::new(LOCK_PATH), b"not a valid lock\n", true)
            .unwrap();
        let err = verify(&fs).unwrap_err();
        assert!(matches!(err, VerifyError::InvalidLock(_)));
    }
}
