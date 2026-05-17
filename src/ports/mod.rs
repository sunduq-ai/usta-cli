//! Trait definitions (a.k.a. *ports* in the hexagonal pattern).
//!
//! Use cases in `crate::app` depend on these traits. Concrete implementations
//! live in `crate::adapters`. `crate::wiring` is the only place where traits
//! and concrete adapters meet (composition root).
//!
//! ## Layer rules
//!
//! - No I/O crates here. Trait method signatures may use `std::path::Path`
//!   for arguments but must not reach into the filesystem.
//! - Each port should follow the Interface Segregation Principle:
//!   keep traits small (≤ ~5 methods). If a trait grows, split it.
//!
//! ## Unused traits
//!
//! Several ports (`pkg_manager`, `sanitizer`, `stack_detector`, `telemetry`,
//! `vcs`) are defined for v0.2 use cases (sandboxed `usta extract` with
//! tree-sitter sanitization, post-scaffold install, anonymous opt-in
//! telemetry, full git wiring). They're intentionally kept compiling now
//! so the v0.2 work is just "fill in adapters", not "redesign ports". The
//! crate-level `dead_code` allow below silences the noise — when v0.2
//! lands, the use cases will consume them and the lint becomes natural.

// See module docs above for rationale.
#![allow(dead_code, unused_imports)]

pub mod clock;
pub mod fs;
pub mod pkg_manager;
pub mod prompts;
pub mod renderer;
pub mod repo_scanner;
pub mod sanitizer;
pub mod stack_detector;
pub mod telemetry;
pub mod template_source;
pub mod vcs;
