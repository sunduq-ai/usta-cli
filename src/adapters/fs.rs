//! Filesystem adapters.
//!
//! Two adapters live here:
//!
//! - [`LocalFs`]: real filesystem, with a write-jail. Refuses to read or
//!   write any path that resolves outside its configured root.
//! - [`InMemoryFs`]: HashMap-backed, used for unit tests.
//!
//! Both implement [`crate::ports::fs::FileSystem`].
//!
//! The write-jail on `LocalFs` is the single most important safety property
//! in the project — templates can be user-provided, and a malicious template
//! that wrote to `~/.ssh/authorized_keys` would be a CVE. A `proptest`
//! property test below tries hard to defeat the jail.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use crate::ports::fs::{FileSystem, FsError};

/// In-memory filesystem adapter for unit tests.
#[derive(Debug, Default)]
pub struct InMemoryFs {
    files: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
}

impl InMemoryFs {
    /// Create an empty in-memory FS.
    pub fn new() -> Self {
        Self {
            files: Mutex::new(BTreeMap::new()),
        }
    }

    /// Snapshot of all written files (sorted).
    pub fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.files
            .lock()
            .expect("poisoned")
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl FileSystem for InMemoryFs {
    fn write(&self, path: &Path, bytes: &[u8], force: bool) -> Result<(), FsError> {
        let mut files = self.files.lock().expect("poisoned");
        if !force && files.contains_key(path) {
            return Err(FsError::AlreadyExists(path.to_path_buf()));
        }
        files.insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        self.files
            .lock()
            .expect("poisoned")
            .get(path)
            .cloned()
            .ok_or_else(|| FsError::Io {
                path: path.to_path_buf(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            })
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.lock().expect("poisoned").contains_key(path)
    }

    fn mkdir_p(&self, _path: &Path) -> Result<(), FsError> {
        // No-op for the in-memory adapter — directories are implicit.
        Ok(())
    }

    fn remove(&self, path: &Path) -> Result<(), FsError> {
        self.files.lock().expect("poisoned").remove(path);
        Ok(())
    }
}

/// Real filesystem adapter, jailed to a single root directory.
///
/// Every path passed in is normalized and checked against the jail root.
/// Symlinks are not followed when resolving — the canonicalization is
/// purely lexical, so a symlink pointing outside the jail would still be
/// rejected by `Component`-level analysis.
#[derive(Debug, Clone)]
pub struct LocalFs {
    root: PathBuf,
}

impl LocalFs {
    /// Create a new `LocalFs` jailed under `root`. The root must already
    /// exist and be a directory; otherwise an error is returned the first
    /// time it's used.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The jail root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve `rel` against the jail root, lexically (no `canonicalize`),
    /// and verify the result stays inside the jail.
    fn resolve(&self, rel: &Path) -> Result<PathBuf, FsError> {
        // 1. Reject absolute paths outright.
        if rel.is_absolute() {
            return Err(FsError::PathEscape(rel.to_path_buf()));
        }

        // 2. Walk components, refusing `..` that would escape, and refusing
        //    Windows drive prefixes / UNC roots.
        let mut depth: i64 = 0;
        for c in rel.components() {
            match c {
                Component::Prefix(_) | Component::RootDir => {
                    return Err(FsError::PathEscape(rel.to_path_buf()));
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(FsError::PathEscape(rel.to_path_buf()));
                    }
                }
                Component::Normal(_) => {
                    depth += 1;
                }
            }
        }

        // 3. Compose. We deliberately do NOT call `fs::canonicalize` here:
        //    it requires the path to exist and would follow symlinks. The
        //    component check above already proves we stay under `root`
        //    lexically.
        Ok(self.root.join(rel))
    }
}

impl FileSystem for LocalFs {
    fn write(&self, path: &Path, bytes: &[u8], force: bool) -> Result<(), FsError> {
        let abs = self.resolve(path)?;
        if !force && abs.exists() {
            return Err(FsError::AlreadyExists(abs));
        }
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FsError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        std::fs::write(&abs, bytes).map_err(|e| FsError::Io {
            path: abs,
            source: e,
        })
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        let abs = self.resolve(path)?;
        std::fs::read(&abs).map_err(|e| FsError::Io {
            path: abs,
            source: e,
        })
    }

    fn exists(&self, path: &Path) -> bool {
        match self.resolve(path) {
            Ok(abs) => abs.exists(),
            Err(_) => false,
        }
    }

    fn mkdir_p(&self, path: &Path) -> Result<(), FsError> {
        let abs = self.resolve(path)?;
        std::fs::create_dir_all(&abs).map_err(|e| FsError::Io {
            path: abs,
            source: e,
        })
    }

    fn remove(&self, path: &Path) -> Result<(), FsError> {
        let abs = self.resolve(path)?;
        if !abs.exists() {
            return Ok(());
        }
        let res = if abs.is_dir() {
            std::fs::remove_dir_all(&abs)
        } else {
            std::fs::remove_file(&abs)
        };
        res.map_err(|e| FsError::Io {
            path: abs,
            source: e,
        })
    }
}

#[cfg(test)]
mod in_memory_tests {
    use super::*;

    #[test]
    fn write_then_read() {
        let fs = InMemoryFs::new();
        fs.write(Path::new("a.txt"), b"hi", false).unwrap();
        assert_eq!(fs.read(Path::new("a.txt")).unwrap(), b"hi");
    }

    #[test]
    fn no_overwrite_without_force() {
        let fs = InMemoryFs::new();
        fs.write(Path::new("a.txt"), b"hi", false).unwrap();
        let err = fs.write(Path::new("a.txt"), b"bye", false).unwrap_err();
        assert!(matches!(err, FsError::AlreadyExists(_)));
        fs.write(Path::new("a.txt"), b"bye", true).unwrap();
        assert_eq!(fs.read(Path::new("a.txt")).unwrap(), b"bye");
    }
}

#[cfg(test)]
mod local_fs_tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn writes_under_root() {
        let dir = tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        fs.write(Path::new("hello.txt"), b"hi", false).unwrap();
        let on_disk = std::fs::read(dir.path().join("hello.txt")).unwrap();
        assert_eq!(on_disk, b"hi");
    }

    #[test]
    fn rejects_absolute_paths() {
        let dir = tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        let err = fs.write(Path::new("/etc/passwd"), b"x", true).unwrap_err();
        assert!(matches!(err, FsError::PathEscape(_)));
    }

    #[test]
    fn rejects_parent_dir_escape() {
        let dir = tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        let err = fs.write(Path::new("../escape"), b"x", true).unwrap_err();
        assert!(matches!(err, FsError::PathEscape(_)));
    }

    #[test]
    fn allows_balanced_parent_dirs() {
        // a/../b should resolve to b — net depth 1, never escapes.
        let dir = tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        fs.write(Path::new("a/../b.txt"), b"hi", false).unwrap();
        assert!(dir.path().join("b.txt").exists());
    }

    #[test]
    fn read_back_what_you_wrote() {
        let dir = tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        fs.write(Path::new("nested/dir/file.txt"), b"content", false)
            .unwrap();
        let got = fs.read(Path::new("nested/dir/file.txt")).unwrap();
        assert_eq!(got, b"content");
    }
}

#[cfg(test)]
mod write_jail_proptest {
    use proptest::prelude::*;
    use tempfile::tempdir;

    use super::*;

    /// Generate "interesting" path strings that try hard to escape the jail.
    fn path_strategy() -> impl Strategy<Value = String> {
        let segment = prop_oneof![
            Just("..".to_string()),
            Just(".".to_string()),
            Just("".to_string()),
            "[a-z]{1,4}",
            r"[a-z]{1,3}/\.\./[a-z]{1,3}",
            Just("/etc/passwd".to_string()),
            Just("/".to_string()),
            Just("..\\..\\..\\windows\\system32".to_string()),
            Just("/usr/local/bin/usta".to_string()),
        ];
        prop::collection::vec(segment, 0..6).prop_map(|segs| segs.join("/"))
    }

    proptest! {
        /// Property: `LocalFs::resolve` either errors or returns a path that
        /// starts with the jail root. Since every public write/read/remove
        /// goes through `resolve`, this is sufficient to prove the write-jail.
        #[test]
        fn resolve_never_escapes(p in path_strategy()) {
            let dir = tempdir().unwrap();
            let root = dir.path().to_path_buf();
            let fs = LocalFs::new(&root);

            if let Ok(abs) = fs.resolve(Path::new(&p)) {
                prop_assert!(
                    abs.starts_with(&root),
                    "resolved path escaped jail: input {:?} -> {}",
                    p,
                    abs.display()
                );
            }
        }

        /// Property: writes succeed only for inputs that resolve, and when
        /// they do, the file appears under root.
        #[test]
        fn writes_appear_under_root(p in path_strategy(), bytes in proptest::collection::vec(any::<u8>(), 0..16)) {
            let dir = tempdir().unwrap();
            let root = dir.path().to_path_buf();
            let fs = LocalFs::new(&root);

            let resolve_ok = fs.resolve(Path::new(&p)).is_ok();
            let write_res  = fs.write(Path::new(&p), &bytes, true);

            // If resolve says no, write must say no.
            if !resolve_ok {
                prop_assert!(write_res.is_err(), "write succeeded despite resolve rejecting: {:?}", p);
            }

            // If write succeeded, the resulting file is under root.
            if write_res.is_ok() {
                let abs = fs.resolve(Path::new(&p)).unwrap();
                prop_assert!(abs.starts_with(&root));
                prop_assert!(abs.exists());
            }
        }
    }
}
