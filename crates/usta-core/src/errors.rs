//! Domain-level error types. Concrete adapter errors live elsewhere.

use thiserror::Error;

/// Errors that can arise from domain rules and pure operations.
#[derive(Debug, Error)]
pub enum DomainError {
    /// A project name failed validation (kebab-case, length, npm rules, etc.).
    #[error("invalid project name: {0}")]
    InvalidProjectName(String),

    /// A feature id is unknown or not declared by the active template.
    #[error("unknown feature: {0}")]
    UnknownFeature(String),

    /// A required feature is missing from the selected set.
    #[error("missing required feature: {required} (needed by {by})")]
    MissingRequiredFeature {
        /// The feature that is missing.
        required: String,
        /// The feature that requires it.
        by: String,
    },

    /// Two selected features conflict.
    #[error("feature conflict: {a} conflicts with {b}")]
    FeatureConflict {
        /// First conflicting feature id.
        a: String,
        /// Second conflicting feature id.
        b: String,
    },

    /// A template manifest is malformed or violates invariants.
    #[error("invalid template manifest: {0}")]
    InvalidManifest(String),
}
