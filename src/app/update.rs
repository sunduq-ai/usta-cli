//! `usta update` use case — re-render the template at its current version
//! against stored answers, with a 3-way merge against the project.
//!
//! Per Write op:
//! - **File doesn't exist on disk**: new file (template grew). Write it.
//! - **Disk hash == lock hash**: file is template-managed and untouched
//!   → safe to overwrite with the freshly rendered content.
//! - **Disk hash differs from lock hash AND new render == disk content**:
//!   user's edits already match what we'd produce. No-op (still bump lock).
//! - **Disk hash differs from lock hash AND new render differs**: conflict.
//!   Write the proposed render to `.usta/proposed/<path>` and leave the
//!   working copy alone. The user inspects + merges manually.
//!
//! For Merge and Inject ops:
//! - Merge: re-apply via [`deep_merge`] (idempotent on already-merged keys;
//!   user-added keys preserved).
//! - Inject: re-apply via [`apply_injections`]. If the marker is gone we
//!   treat it as already-applied and move on.
//!
//! [`deep_merge`]: crate::core::merge::deep_merge
//! [`apply_injections`]: crate::core::inject::apply_injections

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::loaded::LoadedTemplate;
use crate::core::paths::to_forward_slashes;
use crate::core::plan::FileOp;
use crate::core::resolver;
use crate::core::snapshot::{ManagedLock, Snapshot};
use crate::ports::clock::Clock;
use crate::ports::fs::{FileSystem, FsError};
use crate::ports::renderer::TemplateRenderer;
use thiserror::Error;

use super::scaffold::plan_executor::sha256_hex;
use super::scaffold::snapshot::{LOCK_PATH, SNAPSHOT_PATH};
use super::scaffold::{plan_builder, snapshot, ScaffoldError};

/// Where proposed changes for conflicts are written.
pub const PROPOSED_DIR: &str = ".usta/proposed";

/// Errors raised by [`update`].
#[derive(Debug, Error)]
pub enum UpdateError {
    /// Project was not scaffolded by usta.
    #[error("no `.usta/snapshot.toml` found at project root")]
    NoSnapshot,
    /// Snapshot file malformed.
    #[error("invalid snapshot.toml: {0}")]
    InvalidSnapshot(String),
    /// Lock file malformed.
    #[error("invalid managed.lock: {0}")]
    InvalidLock(String),
    /// Domain-rule violation surfaced by the resolver.
    #[error(transparent)]
    Domain(#[from] crate::core::DomainError),
    /// Underlying scaffold engine error.
    #[error(transparent)]
    Scaffold(#[from] ScaffoldError),
    /// FS port returned an error.
    #[error("filesystem: {0}")]
    Fs(String),
}

impl From<FsError> for UpdateError {
    fn from(e: FsError) -> Self {
        UpdateError::Fs(e.to_string())
    }
}

/// Inputs to a single update run.
#[derive(Debug, Default)]
pub struct UpdateRequest {
    /// `usta` CLI version recording the new state.
    pub usta_version: String,
}

/// Outcome of an update run.
#[derive(Debug, Clone, Default)]
pub struct UpdateOutcome {
    /// Files newly added by this update (the template grew).
    pub added: Vec<PathBuf>,
    /// Files overwritten (untouched-by-user, picked up new content).
    pub overwritten: Vec<PathBuf>,
    /// Files identical on disk and in the new render (no-op).
    pub unchanged: Vec<PathBuf>,
    /// Files where the user has local edits that conflict with the new
    /// render. The new render is at `.usta/proposed/<path>`.
    pub conflicts: Vec<PathBuf>,
    /// Files in the prior lock that the new template no longer ships
    /// (orphaned — left on disk for the user to remove).
    pub orphaned: Vec<PathBuf>,
}

impl UpdateOutcome {
    /// Total files inspected.
    pub fn total_inspected(&self) -> usize {
        self.added.len()
            + self.overwritten.len()
            + self.unchanged.len()
            + self.conflicts.len()
            + self.orphaned.len()
    }

    /// Whether the update completed without any conflicts.
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// Run the update use case.
pub fn update<F, R, C>(
    fs: &F,
    renderer: &R,
    clock: &C,
    template: &LoadedTemplate,
    req: UpdateRequest,
) -> Result<UpdateOutcome, UpdateError>
where
    F: FileSystem,
    R: TemplateRenderer,
    C: Clock,
{
    // 1. Read snapshot + prior lock.
    let snapshot = read_snapshot(fs)?;
    let prior_lock = read_lock(fs)?;

    // 2. Resolve the prior feature set against the (possibly newer) template.
    use std::collections::BTreeSet;
    let prior_features: BTreeSet<_> = snapshot.features.iter().cloned().collect();
    let resolved = resolver::resolve(&template.manifest, &prior_features)?;

    regenerate(
        fs,
        renderer,
        clock,
        template,
        resolved,
        snapshot,
        prior_lock,
        req.usta_version,
    )
}

/// Shared regeneration engine for `update` and `add`.
///
/// Re-renders the project from the template for `resolved` features,
/// 3-way-merges each file against the live tree via the lock, then strips
/// any residual anchor markers so no marker ever survives into the user's
/// source. `update` calls this with the project's existing feature set;
/// `add` calls it with the existing set plus the newly requested features.
#[allow(clippy::too_many_arguments)]
pub(crate) fn regenerate<F, R, C>(
    fs: &F,
    renderer: &R,
    clock: &C,
    template: &LoadedTemplate,
    resolved: Vec<crate::core::template::FeatureId>,
    mut snapshot: Snapshot,
    prior_lock: ManagedLock,
    usta_version: String,
) -> Result<UpdateOutcome, UpdateError>
where
    F: FileSystem,
    R: TemplateRenderer,
    C: Clock,
{
    use std::collections::BTreeSet;

    // 3. Build the full plan (base + features) with the original answers.
    let plan = plan_builder::build_plan(template, &resolved, &snapshot.answers, PathBuf::new());

    // 4. Walk the plan; classify and act.
    let mut outcome = UpdateOutcome::default();
    let mut new_lock = ManagedLock::default();
    let mut planned_paths: BTreeSet<PathBuf> = BTreeSet::new();

    for op in &plan.ops {
        match op {
            FileOp::Write { path, contents } => {
                planned_paths.insert(path.clone());

                let new_bytes =
                    render_for_write(renderer, template, &snapshot.answers, path, contents)?;
                let new_hash = sha256_hex(&new_bytes);

                let on_disk = if fs.exists(path) {
                    Some(fs.read(path)?)
                } else {
                    None
                };

                match on_disk {
                    None => {
                        // New file from a grown template.
                        fs.write(path, &new_bytes, true)?;
                        new_lock.files.insert(path.clone(), new_hash);
                        outcome.added.push(path.clone());
                    }
                    Some(disk_bytes) => {
                        let disk_hash = sha256_hex(&disk_bytes);
                        let lock_hash = prior_lock.files.get(path).cloned();

                        if Some(&disk_hash) == lock_hash.as_ref() {
                            // Untouched. Overwrite if there's actually a change.
                            if disk_hash != new_hash {
                                fs.write(path, &new_bytes, true)?;
                                outcome.overwritten.push(path.clone());
                            } else {
                                outcome.unchanged.push(path.clone());
                            }
                            new_lock.files.insert(path.clone(), new_hash);
                        } else if disk_hash == new_hash {
                            // User's edits already match the new render.
                            outcome.unchanged.push(path.clone());
                            new_lock.files.insert(path.clone(), new_hash);
                        } else {
                            // Conflict. Normalize so the proposal path
                            // (which surfaces in user-facing messages and
                            // would otherwise be `.usta/proposed\src/lib.rs`
                            // on Windows) uses forward slashes throughout.
                            let proposed =
                                to_forward_slashes(&PathBuf::from(PROPOSED_DIR).join(path));
                            fs.write(&proposed, &new_bytes, true)?;
                            outcome.conflicts.push(path.clone());
                            // Keep the old lock entry so subsequent verifies
                            // continue to flag the user's modifications.
                            if let Some(h) = lock_hash {
                                new_lock.files.insert(path.clone(), h);
                            }
                        }
                    }
                }
            }
            FileOp::Merge {
                path,
                format,
                value,
            } => {
                planned_paths.insert(path.clone());
                let bytes = apply_merge(fs, path, *format, value)?;
                fs.write(path, &bytes, true)?;
                new_lock.files.insert(path.clone(), sha256_hex(&bytes));
                outcome.overwritten.push(path.clone());
            }
            FileOp::Inject {
                path,
                contributions,
            } => {
                planned_paths.insert(path.clone());
                if !fs.exists(path) {
                    // Inject target missing; can't apply. Surface as conflict.
                    outcome.conflicts.push(path.clone());
                    continue;
                }
                let existing = fs.read(path)?;
                let source = std::str::from_utf8(&existing)
                    .map_err(|e| UpdateError::Fs(format!("not UTF-8: {e}")))?;
                // Render contributions through the engine.
                let rendered: Vec<crate::core::plan::AnchorContribution> = contributions
                    .iter()
                    .map(|c| crate::core::plan::AnchorContribution {
                        marker: c.marker.clone(),
                        content: renderer
                            .render(&c.content, &snapshot.answers)
                            .unwrap_or_else(|_| c.content.clone()),
                    })
                    .collect();
                let injected = crate::core::inject::apply_injections(source, &rendered);
                let bytes = injected.into_bytes();
                fs.write(path, &bytes, true)?;
                new_lock.files.insert(path.clone(), sha256_hex(&bytes));
                outcome.overwritten.push(path.clone());
            }
        }
    }

    // 5. Anything in the prior lock that's not in the plan is orphaned.
    for path in prior_lock.files.keys() {
        if !planned_paths.contains(path) {
            outcome.orphaned.push(path.clone());
            // Preserve the old lock entry so verify still tracks it.
            if let Some(h) = prior_lock.files.get(path) {
                new_lock.files.insert(path.clone(), h.clone());
            }
        }
    }

    // 6. Finalization: strip any residual anchor markers from the files we
    //    wrote this run (added/overwritten/unchanged — never conflicts,
    //    which hold the user's own content). This guarantees markers never
    //    survive into the project regardless of which features are active.
    let wrote: Vec<PathBuf> = outcome
        .added
        .iter()
        .chain(outcome.overwritten.iter())
        .chain(outcome.unchanged.iter())
        .cloned()
        .collect();
    crate::app::scaffold::plan_executor::strip_residual_markers(fs, &wrote, &mut new_lock)?;

    // 7. Persist.
    snapshot.template_version = template.manifest.meta.version.clone();
    snapshot.usta_version = usta_version;
    snapshot.created_at = clock.now_rfc3339();
    snapshot.features = resolved;
    snapshot::write_snapshot(fs, &snapshot, &new_lock).map_err(UpdateError::Scaffold)?;

    Ok(outcome)
}

fn read_snapshot<F: FileSystem>(fs: &F) -> Result<Snapshot, UpdateError> {
    let p = Path::new(SNAPSHOT_PATH);
    if !fs.exists(p) {
        return Err(UpdateError::NoSnapshot);
    }
    let bytes = fs.read(p)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| UpdateError::InvalidSnapshot(format!("not UTF-8: {e}")))?;
    toml::from_str(text).map_err(|e| UpdateError::InvalidSnapshot(e.to_string()))
}

fn read_lock<F: FileSystem>(fs: &F) -> Result<ManagedLock, UpdateError> {
    let p = Path::new(LOCK_PATH);
    if !fs.exists(p) {
        return Ok(ManagedLock::default());
    }
    let bytes = fs.read(p)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| UpdateError::InvalidLock(format!("not UTF-8: {e}")))?;
    ManagedLock::from_text(text).map_err(|e| UpdateError::InvalidLock(e.to_string()))
}

fn render_for_write<R: TemplateRenderer>(
    renderer: &R,
    template: &LoadedTemplate,
    answers: &BTreeMap<String, serde_json::Value>,
    path: &Path,
    contents: &[u8],
) -> Result<Vec<u8>, UpdateError> {
    let is_render = template
        .feature_files
        .values()
        .flat_map(|files| files.iter())
        .chain(template.base_files.iter())
        .find(|f| f.rel_path == path)
        .map(|f| f.content.is_rendered())
        .unwrap_or(false);

    if is_render {
        let source = std::str::from_utf8(contents)
            .map_err(|e| UpdateError::Fs(format!("template not UTF-8: {e}")))?;
        let rendered = renderer
            .render(source, answers)
            .map_err(|e| UpdateError::Fs(format!("render: {e}")))?;
        Ok(rendered.into_bytes())
    } else {
        Ok(contents.to_vec())
    }
}

fn apply_merge<F: FileSystem>(
    fs: &F,
    path: &Path,
    format: crate::core::plan::MergeFormat,
    overlay: &serde_json::Value,
) -> Result<Vec<u8>, UpdateError> {
    use crate::core::merge::{canonicalize_keys, deep_merge};

    let mut current: serde_json::Value = if fs.exists(path) {
        let bytes = fs.read(path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| {
            UpdateError::Fs(format!("merge target `{}` not UTF-8: {e}", path.display()))
        })?;
        match format {
            crate::core::plan::MergeFormat::Json => serde_json::from_str(text)
                .map_err(|e| UpdateError::Fs(format!("parse JSON `{}`: {e}", path.display())))?,
            crate::core::plan::MergeFormat::Toml => toml::from_str(text)
                .map_err(|e| UpdateError::Fs(format!("parse TOML `{}`: {e}", path.display())))?,
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    deep_merge(&mut current, overlay);
    canonicalize_keys(&mut current);

    let out = match format {
        crate::core::plan::MergeFormat::Json => {
            let mut s = serde_json::to_string_pretty(&current)
                .map_err(|e| UpdateError::Fs(format!("emit JSON: {e}")))?;
            s.push('\n');
            s
        }
        crate::core::plan::MergeFormat::Toml => {
            let toml_value: toml::Value = serde_json::from_value(current.clone())
                .map_err(|e| UpdateError::Fs(format!("JSON→TOML convert: {e}")))?;
            toml::to_string_pretty(&toml_value)
                .map_err(|e| UpdateError::Fs(format!("emit TOML: {e}")))?
        }
    };
    Ok(out.into_bytes())
}
