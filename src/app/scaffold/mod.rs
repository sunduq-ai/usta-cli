//! Scaffold use case.
//!
//! Composition:
//!
//! 1. [`plan_builder::build_plan`] — pure: turn a [`LoadedTemplate`] +
//!    resolved feature list + answer context into a [`ScaffoldPlan`].
//! 2. [`plan_executor::execute_plan`] — applies the plan via injected
//!    [`FileSystem`] and [`TemplateRenderer`] ports.
//! 3. [`service::ScaffoldService`] — orchestrates resolve → build → execute
//!    in one call.
//!
//! [`LoadedTemplate`]: crate::core::loaded::LoadedTemplate
//! [`ScaffoldPlan`]: crate::core::plan::ScaffoldPlan
//! [`FileSystem`]: crate::ports::fs::FileSystem
//! [`TemplateRenderer`]: crate::ports::renderer::TemplateRenderer

use crate::core::DomainError;
use crate::ports::fs::FsError;
use crate::ports::renderer::RenderError;
use thiserror::Error;

pub mod plan_builder;
pub mod plan_executor;
pub mod service;
pub mod snapshot;

pub use service::ScaffoldService;

/// Errors returned by scaffold operations.
#[derive(Debug, Error)]
pub enum ScaffoldError {
    /// Domain-rule violation (validation, resolution, …).
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// Filesystem failure surfaced by the FS port.
    #[error("filesystem: {0}")]
    Fs(String),
    /// Renderer failure surfaced by the renderer port.
    #[error("render: {0}")]
    Render(String),
}

impl From<FsError> for ScaffoldError {
    fn from(e: FsError) -> Self {
        ScaffoldError::Fs(e.to_string())
    }
}

impl From<RenderError> for ScaffoldError {
    fn from(e: RenderError) -> Self {
        ScaffoldError::Render(e.to_string())
    }
}
