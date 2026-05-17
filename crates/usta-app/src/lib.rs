//! # usta-app
//!
//! Application services (use cases) for `usta`. Each use case is generic over
//! the ports it needs, so it can be unit-tested with in-memory adapters and
//! wired with real adapters in the binary.
//!
//! ## Layer rules
//!
//! - May depend on `usta-core` and `usta-ports`.
//! - MUST NOT depend on `usta-adapters` (CI enforced).

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod add;
pub mod extract;
pub mod list;
pub mod scaffold;
pub mod update;
pub mod verify;

/// Crate-private helper: does `line` contain the marker `name`?
/// Mirrors the detector logic in `usta_core::inject::detect_marker` but
/// stays here so we don't widen `usta-core`'s public surface.
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
