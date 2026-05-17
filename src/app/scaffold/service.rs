//! `ScaffoldService` — the orchestrator wired in `usta-cli`.
//!
//! Generic over its dependencies so unit tests can compose in-memory fakes
//! and the binary composes real adapters.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::core::loaded::LoadedTemplate;
use crate::core::resolver;
use crate::core::snapshot::Snapshot;
use crate::core::template::FeatureId;
use crate::ports::clock::Clock;
use crate::ports::fs::FileSystem;
use crate::ports::renderer::TemplateRenderer;

use super::{plan_builder, plan_executor, snapshot, ScaffoldError};

/// Inputs to a single scaffold run.
#[derive(Debug)]
pub struct ScaffoldRequest {
    /// Project root (absolute). All file ops are anchored here.
    pub root: PathBuf,
    /// User-selected feature ids (resolver auto-includes their requires).
    pub features: BTreeSet<FeatureId>,
    /// Answer context; available to rendered templates.
    pub answers: BTreeMap<String, serde_json::Value>,
    /// Whether to overwrite existing files.
    pub force: bool,
    /// `usta` CLI version string to record in `.usta/snapshot.toml`.
    pub usta_version: String,
}

/// Run-once result of a scaffold.
#[derive(Debug)]
pub struct ScaffoldOutcome {
    /// Resolved (effective) feature order, including auto-included requires.
    pub resolved_features: Vec<FeatureId>,
    /// Number of files written (excluding the snapshot itself).
    pub files_written: usize,
}

/// Orchestrates resolve → plan-build → execute → snapshot. Generic over
/// ports so tests can pass in fakes.
pub struct ScaffoldService<F, R, C> {
    fs: F,
    renderer: R,
    clock: C,
}

impl<F, R, C> ScaffoldService<F, R, C>
where
    F: FileSystem,
    R: TemplateRenderer,
    C: Clock,
{
    /// Construct a new service from concrete adapters.
    pub fn new(fs: F, renderer: R, clock: C) -> Self {
        Self {
            fs,
            renderer,
            clock,
        }
    }

    /// Run a scaffold.
    pub fn run(
        &self,
        template: &LoadedTemplate,
        req: ScaffoldRequest,
    ) -> Result<ScaffoldOutcome, ScaffoldError> {
        // 1. Resolve features against the manifest.
        let resolved = resolver::resolve(&template.manifest, &req.features)?;

        // 2. Build the plan (pure).
        let plan = plan_builder::build_plan(template, &resolved, &req.answers, req.root.clone());
        let files_written = plan.ops.len();

        // 3. Execute against the FS + renderer; collect the lock.
        let lock = plan_executor::execute_plan(
            &plan,
            template,
            &req.answers,
            &self.fs,
            &self.renderer,
            req.force,
        )?;

        // 4. Persist the snapshot + lock under .usta/.
        let snap = Snapshot {
            template_id: template.manifest.id().clone(),
            template_version: template.manifest.meta.version.clone(),
            usta_version: req.usta_version,
            created_at: self.clock.now_rfc3339(),
            features: resolved.clone(),
            answers: req.answers,
        };
        snapshot::write_snapshot(&self.fs, &snap, &lock)?;

        Ok(ScaffoldOutcome {
            resolved_features: resolved,
            files_written,
        })
    }
}
