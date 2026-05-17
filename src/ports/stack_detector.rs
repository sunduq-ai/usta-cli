//! Stack-detection port. Detectors are composed as a chain.

use std::path::Path;

/// A signal that a particular stack is present in a repository.
#[derive(Debug, Clone)]
pub struct StackHit {
    /// Stack id (e.g. `"typescript"`, `"python"`, `"go"`, `"rust"`).
    pub stack: String,
    /// The file that triggered detection (relative path).
    pub via_file: std::path::PathBuf,
    /// Confidence 0..=100; aggregated by the synthesizer.
    pub confidence: u8,
}

/// A single detector. Returns `None` when the file is not a signal it knows.
pub trait StackDetector: Send + Sync {
    /// Identifier (e.g. `"package_json"`).
    fn id(&self) -> &'static str;

    /// Inspect a file and emit a hit if it matches.
    fn detect(&self, rel_path: &Path, contents: &[u8]) -> Option<StackHit>;
}
