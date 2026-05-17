//! `.gitignore`-respecting repo scanner.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use usta_ports::repo_scanner::{RepoScanner, ScanError, ScannedFile};

/// Walks a repository, honoring `.gitignore` and an optional
/// `.usta-extract-ignore` if present at the repo root.
#[derive(Debug, Default, Clone, Copy)]
pub struct IgnoreScanner;

impl IgnoreScanner {
    /// Construct.
    pub fn new() -> Self {
        Self
    }
}

impl RepoScanner for IgnoreScanner {
    fn scan(&self, root: &Path) -> Result<Vec<ScannedFile>, ScanError> {
        let mut builder = WalkBuilder::new(root);
        builder
            .hidden(false) // keep .gitignore, .prettierrc, etc.
            .git_ignore(true)
            .git_global(false)
            .git_exclude(true)
            .ignore(true)
            .require_git(false) // honor .gitignore even outside a git repo
            .parents(false)
            .follow_links(false);

        // usta-specific ignore file.
        builder.add_custom_ignore_filename(".usta-extract-ignore");

        let mut out: Vec<ScannedFile> = Vec::new();
        for result in builder.build() {
            let entry = result.map_err(|e| ScanError::Io(format!("walk: {e}")))?;
            let p = entry.path();
            // Skip the root itself and any directory entries.
            let file_type = match entry.file_type() {
                Some(ft) => ft,
                None => continue,
            };
            if !file_type.is_file() {
                continue;
            }
            let rel = p
                .strip_prefix(root)
                .map_err(|e| ScanError::Io(format!("strip prefix: {e}")))?
                .to_path_buf();

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(ScannedFile {
                rel_path: rel,
                size,
            });
        }

        // Stable order so downstream synthesis is deterministic.
        out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        Ok(out)
    }
}

/// Read each file scanned and pair it with its bytes. Convenience for
/// callers that need the full content (e.g. extract).
pub fn read_all(
    root: &Path,
    scanned: &[ScannedFile],
) -> Result<Vec<(PathBuf, Vec<u8>)>, ScanError> {
    let mut out = Vec::with_capacity(scanned.len());
    for s in scanned {
        let abs = root.join(&s.rel_path);
        let bytes = std::fs::read(&abs)
            .map_err(|e| ScanError::Io(format!("read {}: {e}", abs.display())))?;
        out.push((s.rel_path.clone(), bytes));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn write(p: &Path, content: &[u8]) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn scans_simple_tree_in_stable_order() {
        let d = tempdir().unwrap();
        write(&d.path().join("README.md"), b"hi");
        write(&d.path().join("src/main.rs"), b"fn main() {}");
        write(&d.path().join("src/lib.rs"), b"pub mod x;");

        let s = IgnoreScanner::new();
        let files = s.scan(d.path()).unwrap();
        let paths: Vec<String> = files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert_eq!(paths, vec!["README.md", "src/lib.rs", "src/main.rs"]);
    }

    #[test]
    fn respects_gitignore() {
        let d = tempdir().unwrap();
        write(&d.path().join(".gitignore"), b"target/\nignored.txt\n");
        write(&d.path().join("kept.txt"), b"keep me");
        write(&d.path().join("ignored.txt"), b"drop me");
        write(&d.path().join("target/build.out"), b"drop me too");

        let s = IgnoreScanner::new();
        let files = s.scan(d.path()).unwrap();
        let paths: Vec<String> = files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert!(paths.contains(&"kept.txt".to_string()));
        assert!(paths.contains(&".gitignore".to_string())); // hidden but kept
        assert!(!paths.iter().any(|p| p == "ignored.txt"));
        assert!(!paths.iter().any(|p| p.starts_with("target/")));
    }

    #[test]
    fn respects_usta_extract_ignore() {
        let d = tempdir().unwrap();
        write(&d.path().join(".usta-extract-ignore"), b"secrets/\n");
        write(&d.path().join("ok.txt"), b"ok");
        write(&d.path().join("secrets/api.key"), b"shhh");

        let s = IgnoreScanner::new();
        let files = s.scan(d.path()).unwrap();
        let paths: Vec<String> = files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert!(paths.contains(&"ok.txt".to_string()));
        assert!(!paths.iter().any(|p| p.starts_with("secrets/")));
    }

    #[test]
    fn read_all_returns_bytes() {
        let d = tempdir().unwrap();
        write(&d.path().join("a.txt"), b"hello");
        let s = IgnoreScanner::new();
        let files = s.scan(d.path()).unwrap();
        let pairs = read_all(d.path(), &files).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, PathBuf::from("a.txt"));
        assert_eq!(pairs[0].1, b"hello");
    }
}
