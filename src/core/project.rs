//! Validated project naming and scope types.

use serde::{Deserialize, Serialize};

use super::errors::DomainError;

/// A validated, kebab-case project name.
///
/// Construction goes through [`ProjectName::parse`], which enforces the npm
/// package-name subset we accept across all stacks: lowercase ASCII letters,
/// digits, and hyphens; 2–214 chars; must start with a letter **or digit**
/// (npm allows a leading digit, e.g. `3d-viewer`) and must not start or end
/// with a hyphen.
///
/// Note: a leading digit is fine for npm/PyPI distribution names and for
/// directory names, but it is *not* a valid bare identifier in every
/// language (Rust crate names and Python module names, for instance, cannot
/// start with a digit). The built-in templates only ever interpolate the
/// name into package-manifest names and display text, never a raw
/// identifier, so this is safe for them. A stack-specific template that uses
/// the name as an identifier should validate that itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectName(String);

impl ProjectName {
    /// Validate and construct.
    pub fn parse(s: impl Into<String>) -> Result<Self, DomainError> {
        let s = s.into();
        if !(2..=214).contains(&s.len()) {
            return Err(DomainError::InvalidProjectName(format!(
                "length {} not in 2..=214",
                s.len()
            )));
        }
        let mut chars = s.chars();
        let first = chars
            .next()
            .ok_or_else(|| DomainError::InvalidProjectName("empty".into()))?;
        if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
            return Err(DomainError::InvalidProjectName(format!(
                "must start with a lowercase letter or digit (got {first:?})"
            )));
        }
        if s.ends_with('-') {
            return Err(DomainError::InvalidProjectName(
                "must not end with a hyphen".into(),
            ));
        }
        for c in chars {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(DomainError::InvalidProjectName(format!(
                    "invalid character: {c:?} (allowed: a-z 0-9 -)"
                )));
            }
        }
        Ok(Self(s))
    }

    /// The validated string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_kebab_case() {
        assert!(ProjectName::parse("my-app").is_ok());
        assert!(ProjectName::parse("usta").is_ok());
        assert!(ProjectName::parse("a1").is_ok());
    }

    #[test]
    fn accepts_leading_digit() {
        // npm allows a leading digit; so do we. This was the `34five`
        // report — a perfectly good name that used to be rejected.
        assert!(ProjectName::parse("34five").is_ok());
        assert!(ProjectName::parse("3d-viewer").is_ok());
        assert!(ProjectName::parse("123").is_ok());
    }

    #[test]
    fn rejects_invalid() {
        assert!(ProjectName::parse("").is_err());
        assert!(ProjectName::parse("X").is_err(), "too short + uppercase");
        assert!(ProjectName::parse("My-App").is_err(), "uppercase");
        assert!(ProjectName::parse("foo_bar").is_err(), "underscore");
        assert!(ProjectName::parse("-lead").is_err(), "leading hyphen");
        assert!(ProjectName::parse("trail-").is_err(), "trailing hyphen");
    }
}
