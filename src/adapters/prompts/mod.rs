//! Prompt adapters.
//!
//! - [`inquire_ui::InquireUi`] — real interactive prompts via `inquire`.
//! - [`noninteractive::NoninteractiveUi`] — for `--yes` and CI; always
//!   returns the supplied default (or errors if no default).

pub mod inquire_ui;
pub mod noninteractive;
