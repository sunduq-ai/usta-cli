//! Application services (use cases) for `usta`. Each use case is generic over
//! the ports it needs, so it can be unit-tested with in-memory adapters and
//! wired with real adapters in `crate::wiring`.
//!
//! ## Layer rules
//!
//! - May depend on `crate::core` and `crate::ports`.
//! - MUST NOT depend on `crate::adapters` (enforced by code review now that
//!   the layers live in one crate; was Cargo-enforced when they were
//!   separate crates).
//!
//! Some service constructors, snapshot methods, and helper functions are
//! intentionally part of the engine's API surface even though the current
//! v0.1 CLI only consumes a subset. Silencing `dead_code` at the module
//! level keeps the surface visible without scattering `#[allow]` on every
//! item.

#![allow(dead_code, unused_imports)]

pub mod add;
pub mod extract;
pub mod list;
pub mod scaffold;
pub mod update;
pub mod verify;

/// Crate-private helper: does `line` contain the marker `name`?
/// Mirrors the detector logic in `crate::core::inject::detect_marker` but
/// stays here so we don't widen `crate::core`'s public surface.
pub(crate) fn __has_marker(line: &str, name: &str) -> bool {
    let trimmed = line.trim_start();
    let body = if let Some(rest) = trimmed.strip_prefix("{/*") {
        rest.trim_start().trim_end_matches("*/}").trim()
    } else if let Some(rest) = trimmed.strip_prefix("<!--") {
        rest.trim_start().trim_end_matches("-->").trim()
    } else if let Some(rest) = trimmed.strip_prefix("//") {
        rest.trim_start()
    } else if let Some(rest) = trimmed.strip_prefix("#") {
        rest.trim_start()
    } else {
        return false;
    };
    body.trim() == name
}
