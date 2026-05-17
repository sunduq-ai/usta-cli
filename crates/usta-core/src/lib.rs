//! # usta-core
//!
//! Pure domain types for the `usta` scaffolding tool.
//!
//! ## Layer rules
//!
//! This crate contains **only** value types, domain rules, and pure functions.
//! It MUST NOT depend on any I/O crate (no `tokio`, no `std::fs`, no
//! `std::process`, no `reqwest`, no `git2`). The CI script
//! `scripts/check-layers.sh` enforces this; do not weaken it.
//!
//! See `docs/ARCHITECTURE.md` in the repository root for the full layering
//! contract.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod errors;
pub mod extract;
pub mod inject;
pub mod loaded;
pub mod merge;
pub mod plan;
pub mod project;
pub mod resolver;
pub mod snapshot;
pub mod template;

pub use errors::DomainError;
