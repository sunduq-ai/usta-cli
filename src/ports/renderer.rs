//! Template-rendering port.

use std::collections::BTreeMap;

use thiserror::Error;

/// Rendering errors.
#[derive(Debug, Error)]
pub enum RenderError {
    /// A template referenced an unknown variable or filter.
    #[error("render error: {0}")]
    Render(String),
    /// Template source was syntactically invalid.
    #[error("template syntax error: {0}")]
    Syntax(String),
}

/// Render a single template string against a context.
///
/// The chosen syntax is implementation-defined but the engine assumes
/// Jinja-compatible delimiters (`{{ var }}`, `{% if %}`, etc.).
pub trait TemplateRenderer {
    /// Render `source` against `context`.
    fn render(
        &self,
        source: &str,
        context: &BTreeMap<String, serde_json::Value>,
    ) -> Result<String, RenderError>;
}
