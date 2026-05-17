//! Interactive prompt port.

use thiserror::Error;

/// Prompt errors surfaced by [`PromptUi`] implementations.
#[derive(Debug, Error)]
pub enum PromptError {
    /// User cancelled (Ctrl-C, ESC).
    #[error("cancelled by user")]
    Cancelled,
    /// Backend failure (TTY, I/O).
    #[error("prompt backend failure: {0}")]
    Backend(String),
}

/// Minimal interactive prompt surface. The non-interactive (CI / `--yes`)
/// adapter implements the same trait but always returns the supplied default.
pub trait PromptUi {
    /// Free text question.
    fn text(&self, question: &str, default: Option<&str>) -> Result<String, PromptError>;

    /// Yes/no question.
    fn confirm(&self, question: &str, default: bool) -> Result<bool, PromptError>;

    /// Single choice.
    fn select(&self, question: &str, options: &[String]) -> Result<usize, PromptError>;

    /// Multi-choice; returns selected indices.
    fn multiselect(
        &self,
        question: &str,
        options: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>, PromptError>;
}
