//! Extract use case — turn an existing repo into a `usta` template.
//!
//! Composition:
//!
//! 1. [`synthesizer::synthesize`] — pure: takes scanned files + an
//!    [`ExtractConfig`] and returns an [`ExtractedTemplate`] (manifest +
//!    files laid out the way they'll appear under `templates/<id>/`).
//! 2. [`writer::write_extracted_template`] — writes that to disk via a
//!    [`FileSystem`] port.
//! 3. [`service::ExtractService`] — orchestrator wired in the binary.
//!
//! [`ExtractConfig`]: usta_core::extract::ExtractConfig
//! [`FileSystem`]: usta_ports::fs::FileSystem

use thiserror::Error;
use usta_ports::fs::FsError;

pub mod service;
pub mod synthesizer;
pub mod writer;

pub use service::{ExtractOutcome, ExtractService};
pub use synthesizer::{synthesize, ExtractedTemplate, TemplateOutFile};

/// Errors returned by extract operations.
#[derive(Debug, Error)]
pub enum ExtractError {
    /// Bad input config (e.g. malformed glob).
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    /// Filesystem failure surfaced by the FS port.
    #[error("filesystem: {0}")]
    Fs(String),
    /// Synthesis failed (e.g. could not serialize manifest).
    #[error("synthesis: {0}")]
    Synthesis(String),
}

impl From<FsError> for ExtractError {
    fn from(e: FsError) -> Self {
        ExtractError::Fs(e.to_string())
    }
}
