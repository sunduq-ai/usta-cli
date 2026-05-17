//! Pure domain types for the `usta` scaffolding tool.
//!
//! ## Layer rules
//!
//! This module contains **only** value types, domain rules, and pure
//! functions. It MUST NOT use any I/O crate (no `tokio`, no `std::fs`, no
//! `std::process`, no `reqwest`, no `git2`). Enforced by code review now
//! that the layers live in one crate; was Cargo-enforced when they were
//! separate crates.
//!
//! See `docs/ARCHITECTURE.md` for the full layering contract.
//!
//! Some helpers (`deep_merge_all`, etc.) are intentionally part of the
//! engine's API surface even though the v0.1 CLI consumes only a subset.
//! Silencing `dead_code` keeps them visible without scattering `#[allow]`.

#![allow(dead_code, unused_imports)]

pub mod errors;
pub mod extract;
pub mod inject;
pub mod loaded;
pub mod merge;
pub mod paths;
pub mod plan;
pub mod project;
pub mod resolver;
pub mod snapshot;
pub mod template;

pub use errors::DomainError;
