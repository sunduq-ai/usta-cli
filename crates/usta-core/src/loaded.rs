//! In-memory representation of a fully-loaded template.
//!
//! `TemplateSource` adapters produce these; use cases consume them. Keeping
//! the type pure means use cases can be unit-tested without a real
//! filesystem.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::plan::{AnchorContribution, MergeFormat};
use crate::template::{FeatureId, Template};

/// A template, its base files, and per-feature files — all in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedTemplate {
    /// Manifest data (parsed from `template.toml`).
    pub manifest: Template,
    /// Files under `base/`, keyed by their final destination path.
    pub base_files: Vec<TemplateFile>,
    /// Per-feature files (under `features/<id>/files/`), keyed by feature id.
    pub feature_files: BTreeMap<FeatureId, Vec<TemplateFile>>,
    /// Per-feature deep-merge contributions (from `features/<id>/merges/`).
    #[serde(default)]
    pub feature_merges: BTreeMap<FeatureId, Vec<MergeFile>>,
    /// Per-feature anchor injections (from `features/<id>/injections/`).
    #[serde(default)]
    pub feature_injections: BTreeMap<FeatureId, Vec<InjectionFile>>,
}

/// One deep-merge contribution into a structured config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeFile {
    /// Destination, relative to the project root (e.g. `package.json`).
    pub target: PathBuf,
    /// Source format (we re-emit in the same format).
    pub format: MergeFormat,
    /// Already-parsed merge value (in JSON-shape regardless of source).
    pub value: serde_json::Value,
}

/// One feature's contributions to anchor markers in a single target file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionFile {
    /// Destination, relative to the project root.
    pub target: PathBuf,
    /// Ordered list of (marker, content) pairs to insert.
    pub contributions: Vec<AnchorContribution>,
}

/// A single file in a template, with its destination path and content kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    /// Path relative to the generated project root.
    /// `.j2` extensions have been stripped by the loader.
    pub rel_path: PathBuf,
    /// File body.
    pub content: TemplateContent,
}

/// Two ways a template file's body can be sourced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateContent {
    /// Plain bytes — copied verbatim. Binary-safe.
    Verbatim(Vec<u8>),
    /// Jinja source. Rendered against the answer context at write time.
    Render(String),
}

impl TemplateContent {
    /// Returns true if this file should be passed through the renderer.
    pub fn is_rendered(&self) -> bool {
        matches!(self, TemplateContent::Render(_))
    }
}
