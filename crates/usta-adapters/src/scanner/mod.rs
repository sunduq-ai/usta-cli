//! Repo-scanner adapters.
//!
//! - [`ignore_scanner::IgnoreScanner`] — `.gitignore`-respecting walker
//!   built on top of the `ignore` crate (the same one ripgrep uses). It
//!   also picks up `.usta-extract-ignore` for usta-specific exclusions.

pub mod ignore_scanner;
