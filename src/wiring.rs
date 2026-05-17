//! Composition root.
//!
//! This is the **only** module outside `crate::adapters` allowed to mention
//! concrete adapter types. `crate::commands::*` may also instantiate adapters
//! directly — they are part of the binary, not the use-case layer, and the
//! exemption keeps simple read-only commands (e.g. `usta list`) from needing
//! a wiring helper each. The layer rule used to be Cargo-enforced when this
//! lived in `usta-cli`; since the v0.1.0 single-crate collapse it's a code
//! review responsibility. See `docs/ARCHITECTURE.md` and
//! `docs/ADR/0002-single-crate-collapse.md`.

use std::path::PathBuf;

use crate::adapters::clock::SystemClock;
use crate::adapters::fs::LocalFs;
use crate::adapters::prompts::inquire_ui::InquireUi;
use crate::adapters::prompts::noninteractive::NoninteractiveUi;
use crate::adapters::renderer::MinijinjaRenderer;
use crate::adapters::templates::filesystem_source::FilesystemTemplateSource;
use crate::app::scaffold::ScaffoldService;

/// Build a `ScaffoldService` wired with the local FS + minijinja renderer
/// + system clock.
pub fn build_scaffold_service(
    project_root: PathBuf,
) -> ScaffoldService<LocalFs, MinijinjaRenderer, SystemClock> {
    let fs = LocalFs::new(project_root);
    let renderer = MinijinjaRenderer::new();
    let clock = SystemClock::new();
    ScaffoldService::new(fs, renderer, clock)
}

/// Build a filesystem template source rooted at `dir`.
pub fn build_template_source(dir: PathBuf) -> FilesystemTemplateSource {
    FilesystemTemplateSource::new(dir)
}

/// Choose interactive vs. non-interactive UI based on the `--yes` flag.
/// Returned as a boxed trait object so the call site stays polymorphic.
pub fn build_prompt_ui(non_interactive: bool) -> Box<dyn crate::ports::prompts::PromptUi> {
    if non_interactive {
        Box::new(NoninteractiveUi)
    } else {
        Box::new(InquireUi::new())
    }
}
