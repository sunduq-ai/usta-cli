//! Concrete implementations of every trait in `crate::ports`. Each module
//! corresponds to one adapter. Tests live next to each adapter and exercise
//! the real backend (filesystem, child process, …) using `tempfile` for
//! isolation.
//!
//! ## Layer rules
//!
//! - May depend on third-party I/O crates freely (this is the only place).
//! - MUST NOT be imported by `crate::app`. `crate::wiring` is the only
//!   allowed importer (enforced by code review now that the layers live in
//!   one crate; was Cargo-enforced when they were separate crates).
//!
//! Some adapter constructors are part of the engine's API surface and not
//! all of them are wired in the v0.1 CLI yet (e.g. `InMemoryFs` is used by
//! some tests but not by the binary's hot path). Silencing `dead_code` at
//! the module level keeps these visible.

#![allow(dead_code, unused_imports)]

pub mod clock;
pub mod fs;
pub mod pkg_manager;
pub mod prompts;
pub mod renderer;
pub mod scanner;
pub mod templates;
pub mod vcs;
