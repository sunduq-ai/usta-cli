//! # usta-ports
//!
//! Trait definitions (a.k.a. *ports* in the hexagonal pattern).
//!
//! Use cases in `usta-app` depend on these traits. Concrete implementations
//! live in `usta-adapters`. The binary crate `usta-cli` is the only place
//! where traits and concrete adapters meet (composition root).
//!
//! ## Layer rules
//!
//! - No I/O crates here. Trait method signatures may use `std::path::Path`
//!   for arguments but must not reach into the filesystem.
//! - Each port should follow the Interface Segregation Principle:
//!   keep traits small (≤ ~5 methods). If a trait grows, split it.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

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
