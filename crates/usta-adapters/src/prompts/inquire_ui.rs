//! Interactive prompt adapter using `inquire`.

use inquire::ui::{RenderConfig, Styled};
use inquire::{Confirm, MultiSelect, Select, Text};
use usta_ports::prompts::{PromptError, PromptUi};

/// Real interactive UI. Wraps `inquire`. Maps `inquire::InquireError::OperationCanceled`
/// (and similar) to [`PromptError::Cancelled`] so the binary can exit with
/// the documented exit-code 3.
///
/// Customizes inquire's [`RenderConfig`] to drop the default leading `?`
/// glyph — questions read more naturally as English when the `?`
/// belongs at the end of the sentence (e.g. `Project description?`)
/// rather than at the start (e.g. `? Project description`).
/// We pair this with an internal `format_question` helper that appends
/// a `?` to the question text when it doesn't already terminate.
#[derive(Debug, Default)]
pub struct InquireUi;

impl InquireUi {
    /// Construct.
    pub fn new() -> Self {
        Self
    }

    /// Build a render config that drops the leading `?` prefix while
    /// keeping inquire's default colors / suffix arrow.
    fn render_config() -> RenderConfig<'static> {
        RenderConfig::default_colored().with_prompt_prefix(Styled::new(""))
    }
}

/// Append `?` to a question if it doesn't already end with sentence-ending
/// punctuation. Strips trailing whitespace first so we don't end up with
/// double spaces.
fn format_question(q: &str) -> String {
    let trimmed = q.trim_end();
    let last = trimmed.chars().last();
    match last {
        Some('?') | Some('!') | Some(':') | Some('.') | None => trimmed.to_string(),
        _ => format!("{trimmed}?"),
    }
}

fn map_err(e: inquire::InquireError) -> PromptError {
    match e {
        inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted => {
            PromptError::Cancelled
        }
        other => PromptError::Backend(other.to_string()),
    }
}

impl PromptUi for InquireUi {
    fn text(&self, question: &str, default: Option<&str>) -> Result<String, PromptError> {
        let q = format_question(question);
        let mut p = Text::new(&q).with_render_config(Self::render_config());
        if let Some(d) = default {
            p = p.with_default(d);
        }
        p.prompt().map_err(map_err)
    }

    fn confirm(&self, question: &str, default: bool) -> Result<bool, PromptError> {
        let q = format_question(question);
        Confirm::new(&q)
            .with_default(default)
            .with_render_config(Self::render_config())
            .prompt()
            .map_err(map_err)
    }

    fn select(&self, question: &str, options: &[String]) -> Result<usize, PromptError> {
        if options.is_empty() {
            return Err(PromptError::Backend("select with no options".into()));
        }
        let q = format_question(question);
        let opts: Vec<&str> = options.iter().map(String::as_str).collect();
        let chosen = Select::new(&q, opts)
            .with_render_config(Self::render_config())
            .prompt()
            .map_err(map_err)?;
        Ok(options.iter().position(|s| s == chosen).unwrap_or(0))
    }

    fn multiselect(
        &self,
        question: &str,
        options: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>, PromptError> {
        if options.is_empty() {
            return Ok(Vec::new());
        }
        let default_indices: Vec<usize> = defaults
            .iter()
            .enumerate()
            .filter_map(|(i, &b)| if b { Some(i) } else { None })
            .collect();
        let q = format_question(question);
        let opts: Vec<&str> = options.iter().map(String::as_str).collect();
        let chosen = MultiSelect::new(&q, opts)
            .with_default(&default_indices)
            .with_render_config(Self::render_config())
            .prompt()
            .map_err(map_err)?;
        Ok(chosen
            .into_iter()
            .filter_map(|s| options.iter().position(|o| o == s))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::format_question;

    #[test]
    fn appends_question_mark_when_missing() {
        assert_eq!(
            format_question("Project description"),
            "Project description?"
        );
    }

    #[test]
    fn preserves_existing_terminator() {
        assert_eq!(format_question("Are you sure?"), "Are you sure?");
        assert_eq!(format_question("Pick one:"), "Pick one:");
        assert_eq!(format_question("Heads up!"), "Heads up!");
        assert_eq!(format_question("Done."), "Done.");
    }

    #[test]
    fn handles_trailing_whitespace() {
        assert_eq!(format_question("Project name   "), "Project name?");
        assert_eq!(format_question("Are you sure?  "), "Are you sure?");
    }

    #[test]
    fn empty_string_stays_empty() {
        assert_eq!(format_question(""), "");
        assert_eq!(format_question("   "), "");
    }
}
