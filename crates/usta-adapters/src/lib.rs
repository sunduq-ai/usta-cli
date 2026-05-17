//! # usta-adapters
//!
//! Concrete implementations of every trait in `usta-ports`. Each module
//! corresponds to one adapter. Tests live next to each adapter and exercise
//! the real backend (filesystem, child process, …) using `tempfile` for
//! isolation.
//!
//! ## Layer rules
//!
//! - May depend on third-party I/O crates freely (this is the only place).
//! - MUST NOT be imported by `usta-app`. The binary `usta-cli` is the
//!   only allowed importer.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod clock;
pub mod fs;
pub mod prompts;
pub mod renderer;
pub mod scanner;
pub mod templates;
