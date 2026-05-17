//! Package-manager port. One adapter per ecosystem (pnpm, uv, cargo, go, …).

use std::path::Path;

use thiserror::Error;

/// Package-manager errors.
#[derive(Debug, Error)]
pub enum PkgError {
    /// The required tool was not found on `$PATH`.
    #[error("package manager not found on PATH: {0}")]
    NotFound(String),
    /// The tool exited with a non-zero status.
    #[error("{tool} failed with exit code {code}: {message}")]
    Failed {
        /// Tool name.
        tool: String,
        /// Process exit code.
        code: i32,
        /// Tool output.
        message: String,
    },
}

/// A package manager that can install dependencies inside a project tree.
pub trait PackageManager {
    /// Identifier (e.g. `"pnpm"`, `"uv"`, `"cargo"`).
    fn id(&self) -> &'static str;

    /// Whether the tool is available on `$PATH`.
    fn is_available(&self) -> bool;

    /// Install dependencies inside `cwd`.
    fn install(&self, cwd: &Path) -> Result<(), PkgError>;
}
