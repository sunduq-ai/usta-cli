//! Scaffold plan: an ordered, declarative description of file operations.
//!
//! Plans are produced by `usta-app::scaffold::plan_builder` and then executed
//! by `usta-app::scaffold::plan_executor` against a `FileSystem` port.
//! Building a plan is pure; executing it is the only side-effecting step.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A single operation in a scaffold plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOp {
    /// Write `contents` to `path`, creating parent directories as needed.
    Write {
        /// Destination path, relative to the project root.
        path: PathBuf,
        /// File body, already rendered.
        contents: Vec<u8>,
    },
    /// Deep-merge structured `value` into the JSON/TOML file at `path`.
    Merge {
        /// Destination path, relative to the project root.
        path: PathBuf,
        /// Format used for the merge.
        format: MergeFormat,
        /// Already-parsed value to merge in.
        value: serde_json::Value,
    },
    /// Replace anchor markers in `path` with accumulated `contributions`.
    Inject {
        /// Destination path, relative to the project root.
        path: PathBuf,
        /// Map of marker name to ordered contributions.
        contributions: Vec<AnchorContribution>,
    },
}

/// Format used by [`FileOp::Merge`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergeFormat {
    /// JSON document (e.g. `package.json`).
    Json,
    /// TOML document (e.g. `pyproject.toml`).
    Toml,
}

/// One contribution to an anchor marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorContribution {
    /// Anchor identifier (e.g. `"usta:routers"`).
    pub marker: String,
    /// Text inserted at this anchor (preserves order across contributions).
    pub content: String,
}

/// A complete scaffold plan, ready to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldPlan {
    /// Project root, absolute, where all `FileOp::path`s are anchored.
    pub root: PathBuf,
    /// Ordered file operations.
    pub ops: Vec<FileOp>,
}
