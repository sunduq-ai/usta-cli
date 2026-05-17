//! Version-control port.

use std::path::Path;

use thiserror::Error;

/// VCS errors.
#[derive(Debug, Error)]
pub enum VcsError {
    /// VCS tool not on `$PATH`.
    #[error("vcs tool not found: {0}")]
    NotFound(String),
    /// Underlying VCS command failed.
    #[error("vcs command failed: {0}")]
    Failed(String),
}

/// VCS operations needed by `usta new` post-actions.
pub trait VcsClient {
    /// Initialize a repository at `cwd`.
    fn init(&self, cwd: &Path) -> Result<(), VcsError>;

    /// Stage all changes.
    fn add_all(&self, cwd: &Path) -> Result<(), VcsError>;

    /// Create an initial commit.
    fn commit(&self, cwd: &Path, message: &str) -> Result<(), VcsError>;
}
