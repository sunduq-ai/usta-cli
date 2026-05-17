//! Validated project naming and scope types.

use serde::{Deserialize, Serialize};

use crate::errors::DomainError;

/// A validated, kebab-case project name.
///
/// Construction goes through [`ProjectName::parse`], which enforces the npm
/// package-name subset we accept across all stacks: lowercase ASCII letters,
/// digits, and hyphens; must start with a letter; 2–214 chars.
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
        if !first.is_ascii_lowercase() {
            return Err(DomainError::InvalidProjectName(
                "must start with a lowercase ASCII letter".into(),
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
    fn rejects_invalid() {
        assert!(ProjectName::parse("").is_err());
        assert!(ProjectName::parse("X").is_err());
        assert!(ProjectName::parse("My-App").is_err());
        assert!(ProjectName::parse("1abc").is_err());
        assert!(ProjectName::parse("foo_bar").is_err());
    }
}
