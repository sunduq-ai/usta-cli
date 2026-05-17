//! Snapshot data types persisted under `.usta/` in a generated project.
//!
//! Two files are written:
//!
//! - `.usta/snapshot.toml` — what was scaffolded, with which answers, from
//!   which template version. This is what `usta update` uses to re-render.
//! - `.usta/managed.lock` — SHA-256 of every file the template wrote, so
//!   `usta verify` (P4) can detect drift, and `usta update` (P4) can tell
//!   "user-edited" apart from "untouched, safe to overwrite".

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::template::{FeatureId, TemplateId};

/// Persistent record of "what was scaffolded".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Template the project was scaffolded from.
    pub template_id: TemplateId,
    /// Template version at scaffold time.
    pub template_version: semver::Version,
    /// `usta` CLI version that wrote this snapshot.
    pub usta_version: String,
    /// RFC 3339 wall-clock timestamp.
    pub created_at: String,
    /// Effective feature order (post-resolution).
    pub features: Vec<FeatureId>,
    /// Answer map (templated against the template at scaffold time).
    pub answers: BTreeMap<String, serde_json::Value>,
}

/// Lockfile of template-managed files, keyed by path → SHA-256 hex digest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManagedLock {
    /// Path (relative to project root) → digest.
    pub files: BTreeMap<PathBuf, String>,
}

impl ManagedLock {
    /// Render to the on-disk format: one line per file, `<sha256>  <path>`.
    /// Stable order (BTreeMap is sorted).
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# usta managed-files lockfile\n");
        out.push_str("# format: <sha256>  <path>\n");
        for (path, digest) in &self.files {
            out.push_str(digest);
            out.push_str("  ");
            out.push_str(&path.to_string_lossy());
            out.push('\n');
        }
        out
    }

    /// Parse the on-disk format. Skips comments and blank lines.
    pub fn from_text(text: &str) -> Result<Self, ManagedLockParseError> {
        let mut files = BTreeMap::new();
        for (lineno, raw) in text.lines().enumerate() {
            let line = raw.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (digest, path) =
                line.split_once("  ")
                    .ok_or_else(|| ManagedLockParseError::Malformed {
                        line: lineno + 1,
                        content: line.to_string(),
                    })?;
            if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ManagedLockParseError::BadDigest {
                    line: lineno + 1,
                    digest: digest.to_string(),
                });
            }
            files.insert(PathBuf::from(path), digest.to_string());
        }
        Ok(Self { files })
    }
}

/// Errors when parsing a `managed.lock` file.
#[derive(Debug, thiserror::Error)]
pub enum ManagedLockParseError {
    /// A line did not match `<sha256>  <path>`.
    #[error("malformed line {line}: {content}")]
    Malformed {
        /// 1-based line number.
        line: usize,
        /// Offending text.
        content: String,
    },
    /// A digest was not 64 hex characters.
    #[error("bad digest at line {line}: {digest}")]
    BadDigest {
        /// 1-based line number.
        line: usize,
        /// Offending digest.
        digest: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_round_trips_through_text() {
        let mut lock = ManagedLock::default();
        let h = "a".repeat(64);
        lock.files.insert(PathBuf::from("README.md"), h.clone());
        lock.files
            .insert(PathBuf::from("apps/api/main.py"), h.clone());

        let text = lock.to_text();
        let parsed = ManagedLock::from_text(&text).unwrap();

        assert_eq!(parsed.files.len(), 2);
        assert_eq!(parsed.files[&PathBuf::from("README.md")], h);
    }

    #[test]
    fn malformed_line_errors() {
        let bad = "# header\nnot-a-valid-line\n";
        let err = ManagedLock::from_text(bad).unwrap_err();
        assert!(matches!(err, ManagedLockParseError::Malformed { .. }));
    }

    #[test]
    fn bad_digest_errors() {
        // 3-char digest (too short) instead of 64.
        let bad = "# header\nabc  README.md\n";
        let err = ManagedLock::from_text(bad).unwrap_err();
        assert!(matches!(err, ManagedLockParseError::BadDigest { .. }));
    }

    #[test]
    fn empty_text_yields_empty_lock() {
        let lock = ManagedLock::from_text("").unwrap();
        assert!(lock.files.is_empty());
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let text = "\n# hi\n# format: <sha256>  <path>\n\n";
        let lock = ManagedLock::from_text(text).unwrap();
        assert!(lock.files.is_empty());
    }
}
