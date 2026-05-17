//! Non-interactive prompt adapter — for `--yes` and CI runs.
//!
//! Returns the supplied default for every question. Errors if no default
//! exists for a text prompt (the call site should always provide one for
//! prompts that need to run in this mode).

use usta_ports::prompts::{PromptError, PromptUi};

/// Always-defaults prompt UI. Stateless.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoninteractiveUi;

impl NoninteractiveUi {
    /// Construct.
    pub fn new() -> Self {
        Self
    }
}

impl PromptUi for NoninteractiveUi {
    fn text(&self, question: &str, default: Option<&str>) -> Result<String, PromptError> {
        default.map(|s| s.to_string()).ok_or_else(|| {
            PromptError::Backend(format!(
                "non-interactive run: text prompt `{question}` has no default"
            ))
        })
    }

    fn confirm(&self, _question: &str, default: bool) -> Result<bool, PromptError> {
        Ok(default)
    }

    fn select(&self, question: &str, options: &[String]) -> Result<usize, PromptError> {
        if options.is_empty() {
            Err(PromptError::Backend(format!(
                "non-interactive: select `{question}` has no options"
            )))
        } else {
            Ok(0)
        }
    }

    fn multiselect(
        &self,
        _question: &str,
        options: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>, PromptError> {
        Ok(options
            .iter()
            .enumerate()
            .filter_map(|(i, _)| {
                if defaults.get(i).copied().unwrap_or(false) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_text_default() {
        let ui = NoninteractiveUi;
        assert_eq!(ui.text("?", Some("hi")).unwrap(), "hi");
    }

    #[test]
    fn errors_without_text_default() {
        let ui = NoninteractiveUi;
        assert!(ui.text("?", None).is_err());
    }

    #[test]
    fn returns_confirm_default() {
        let ui = NoninteractiveUi;
        assert!(ui.confirm("?", true).unwrap());
        assert!(!ui.confirm("?", false).unwrap());
    }

    #[test]
    fn select_returns_first_option() {
        let ui = NoninteractiveUi;
        let opts = vec!["a".to_string(), "b".to_string()];
        assert_eq!(ui.select("?", &opts).unwrap(), 0);
    }

    #[test]
    fn multiselect_picks_default_indices() {
        let ui = NoninteractiveUi;
        let opts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let defaults = [true, false, true];
        assert_eq!(ui.multiselect("?", &opts, &defaults).unwrap(), vec![0, 2]);
    }
}
