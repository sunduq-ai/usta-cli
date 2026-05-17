//! Source-sanitizer port. One adapter per language; tree-sitter–backed in
//! practice. Used by `usta extract` to strip business code while keeping
//! infrastructure shape.

use thiserror::Error;

/// Sanitizer errors.
#[derive(Debug, Error)]
pub enum SanitizeError {
    /// Could not parse the file with the configured grammar.
    #[error("parse error: {0}")]
    Parse(String),
}

/// Strip the body of every function/method, drop unused imports, and replace
/// project-specific identifiers with template placeholders.
pub trait SourceSanitizer: Send + Sync {
    /// Language id (e.g. `"typescript"`, `"python"`).
    fn language(&self) -> &'static str;

    /// Whether this sanitizer accepts the given file extension.
    fn accepts(&self, extension: &str) -> bool;

    /// Sanitize `source`. Identifier replacements come from `replacements`
    /// (literal substring match, longest-first to avoid partial overlaps).
    fn sanitize(
        &self,
        source: &str,
        replacements: &[(String, String)],
    ) -> Result<String, SanitizeError>;
}
