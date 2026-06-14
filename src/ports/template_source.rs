//! Template-loading port.
//!
//! Adapters implement this for each source kind:
//! - filesystem (a path under `templates/`)
//! - embedded (compiled into the binary via `include_dir`) — planned
//! - cached (a community template under `~/.usta/templates/<id>`) — planned
//!
//! Composing several adapters into one is a simple delegating wrapper that
//! also lives in `crate::adapters`.

use crate::core::loaded::LoadedTemplate;
use crate::core::template::TemplateId;
use thiserror::Error;

/// Errors returned by a [`TemplateSource`].
#[derive(Debug, Error)]
pub enum TemplateSourceError {
    /// The id was not found.
    #[error("unknown template: {0}")]
    NotFound(String),
    /// The manifest failed to parse / validate.
    #[error("invalid manifest for `{id}`: {message}")]
    InvalidManifest {
        /// Template id.
        id: String,
        /// Error message.
        message: String,
    },
    /// Generic I/O while reading template files.
    #[error("io while loading `{id}`: {message}")]
    Io {
        /// Template id.
        id: String,
        /// Error message.
        message: String,
    },
}

/// A source that can list and load templates by id.
pub trait TemplateSource: Send + Sync {
    /// All template ids this source knows about. Order is implementation-
    /// defined; callers that need a stable order should sort.
    fn list_ids(&self) -> Vec<TemplateId>;

    /// Load a template fully into memory.
    fn load(&self, id: &TemplateId) -> Result<LoadedTemplate, TemplateSourceError>;
}
