//! Template, feature, and manifest value types.
//!
//! These are populated by adapters that read disk/registry, then handed to
//! `crate::app` use cases. They contain no I/O and no behavior beyond pure
//! validation helpers.

use serde::{Deserialize, Serialize};

/// Stable identifier for a template (kebab-case).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TemplateId(pub String);

/// Stable identifier for a feature within a template (kebab-case).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FeatureId(pub String);

/// A loaded template manifest. Constructed by adapters; consumed by use cases.
///
/// The on-disk TOML shape:
///
/// ```toml
/// [template]
/// id           = "nx-monorepo"
/// display_name = "Nx Monorepo"
/// version      = "1.0.0"
/// min_usta     = ">=0.1.0"
/// stacks       = ["typescript", "python"]
///
/// [[features]]
/// id = "api-fastapi"
/// # …
///
/// [[prompts]]
/// id = "scope"
/// # …
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Metadata block.
    #[serde(rename = "template")]
    pub meta: TemplateMeta,
    /// Features that can be opted into.
    #[serde(default)]
    pub features: Vec<Feature>,
    /// Prompts run during `usta new`.
    #[serde(default)]
    pub prompts: Vec<Prompt>,
}

impl Template {
    /// Convenience: the template id.
    pub fn id(&self) -> &TemplateId {
        &self.meta.id
    }

    /// Convenience: the display name.
    pub fn display_name(&self) -> &str {
        &self.meta.display_name
    }
}

/// Metadata block of a template manifest. Lives under `[template]` in TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMeta {
    /// Unique identifier (kebab-case).
    pub id: TemplateId,
    /// Human-readable name shown in `usta list`.
    pub display_name: String,
    /// SemVer version of the template definition.
    pub version: semver::Version,
    /// Minimum `usta` CLI version required.
    pub min_usta: semver::VersionReq,
    /// Declared stacks (informational only — engine is stack-agnostic).
    #[serde(default)]
    pub stacks: Vec<String>,
}

/// A toggleable feature within a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    /// Unique identifier within the template.
    pub id: FeatureId,
    /// Human-readable name.
    pub display_name: String,
    /// Whether this feature is selected by default.
    pub default: bool,
    /// Other features this feature depends on.
    pub requires: Vec<FeatureId>,
    /// Other features that conflict with this one.
    pub conflicts: Vec<FeatureId>,
    /// Stacks this feature targets (informational).
    pub stacks: Vec<String>,
}

/// A prompt definition shown to the user during scaffolding.
///
/// On disk (TOML) the shape is intentionally flat to keep authoring simple:
///
/// ```toml
/// [[prompts]]
/// id       = "framework"
/// type     = "select"
/// question = "Pick a framework"
/// options  = ["react", "vue", "svelte"]
/// default  = "react"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Identifier used as the answer key.
    pub id: String,
    /// Prompt question text.
    pub question: String,
    /// Prompt kind.
    #[serde(rename = "type")]
    pub kind: PromptKind,
    /// Default value (templated against prior answers).
    pub default: Option<String>,
    /// Optional regex validation for text prompts.
    pub validate: Option<String>,
    /// Options for `select` / `multiselect` prompts. Empty otherwise.
    #[serde(default)]
    pub options: Vec<String>,
}

/// The kind of input a prompt accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    /// Free text.
    Text,
    /// Yes/no.
    Confirm,
    /// Single choice from `options`.
    Select,
    /// Multi-choice from `options`.
    Multiselect,
}
