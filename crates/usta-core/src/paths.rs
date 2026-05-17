//! Cross-platform relative-path normalization.
//!
//! `usta` writes relative paths into `.usta/managed.lock`,
//! `.usta/snapshot.toml`, and a project's anchor markers. Those files MUST
//! be portable — a project scaffolded on Windows must still `verify`
//! cleanly on macOS/Linux and vice versa.
//!
//! Every adapter that derives a relative path from a filesystem walk
//! (`walkdir`, `ignore`, …) MUST normalize that path through
//! [`to_forward_slashes`] before storing it in any value that crosses the
//! adapter boundary. Downstream code can then assume one canonical form.
//!
//! This module is pure — no I/O, no allocations beyond the returned
//! `PathBuf` — so it belongs in `usta-core` despite operating on paths.

use std::path::{Component, Path, PathBuf};

/// Rewrite a relative path so every separator is `/`, regardless of host OS.
///
/// Behavior is identical across platforms: we always walk the path's
/// [`Component`]s and join their string forms with `/`. This is intentional
/// — a per-OS shortcut (e.g. early-return when `MAIN_SEPARATOR == '/'`)
/// would skip the `.` / `..` normalization below and produce divergent
/// results between Unix and Windows.
///
/// Only normal path components (file/directory names) and `..` are emitted.
/// `.` components are dropped. The caller is expected to pass relative
/// paths; root/prefix components are dropped on the assumption they're
/// meaningless here, with a debug assertion to surface programmer error.
///
/// Pure. Idempotent.
pub fn to_forward_slashes(p: &Path) -> PathBuf {
    let mut parts: Vec<String> = Vec::new();
    for comp in p.components() {
        match comp {
            Component::Normal(os) => parts.push(os.to_string_lossy().into_owned()),
            Component::CurDir => {} // skip `.`
            Component::ParentDir => parts.push("..".into()),
            // Root / prefix / RootDir shouldn't appear in a relative path.
            // In release builds we silently drop them; in debug builds we
            // assert so the bug surfaces.
            Component::Prefix(_) | Component::RootDir => {
                debug_assert!(false, "expected relative path, got: {}", p.display());
            }
        }
    }
    PathBuf::from(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_path_unchanged() {
        let p = Path::new("src/lib.rs");
        assert_eq!(to_forward_slashes(p), PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn empty_path_stays_empty() {
        assert_eq!(to_forward_slashes(Path::new("")), PathBuf::from(""));
    }

    #[test]
    fn single_component_unchanged() {
        assert_eq!(
            to_forward_slashes(Path::new("README.md")),
            PathBuf::from("README.md")
        );
    }

    #[test]
    fn join_then_normalize_uses_forward_slashes() {
        // `join` uses the platform separator under the hood — so on Windows
        // this builds `src\lib.rs` and we expect `src/lib.rs` back. On Unix
        // both sides are already forward-slashed so the equality still
        // holds.
        let joined = Path::new("src").join("nested").join("lib.rs");
        assert_eq!(
            to_forward_slashes(&joined),
            PathBuf::from("src/nested/lib.rs")
        );
    }

    #[test]
    fn idempotent() {
        let p = Path::new("a/b/c");
        let once = to_forward_slashes(p);
        let twice = to_forward_slashes(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn parent_dir_preserved() {
        let p = Path::new("..").join("sibling").join("file.txt");
        assert_eq!(to_forward_slashes(&p), PathBuf::from("../sibling/file.txt"));
    }

    #[test]
    fn current_dir_components_stripped() {
        // `./foo` should normalize to `foo` — leading `.` is noise.
        let p = Path::new(".").join("foo").join("bar");
        assert_eq!(to_forward_slashes(&p), PathBuf::from("foo/bar"));
    }
}
