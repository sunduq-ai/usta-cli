//! Template-source adapters.
//!
//! Currently:
//! - [`filesystem_source::FilesystemTemplateSource`] — loads templates from
//!   a directory containing `<id>/template.toml` per template.
//!
//! Future: `EmbeddedTemplateSource` (compiled-in via `include_dir`),
//! `CachedTemplateSource` (`~/.usta/templates`), and a `CompositeSource`
//! that chains them.

pub mod filesystem_source;
