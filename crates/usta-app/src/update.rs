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
//! [`deep_merge`]: usta_core::merge::deep_merge
//! [`apply_injections`]: usta_core::inject::apply_injections

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;
use usta_core::loaded::LoadedTemplate;
use usta_core::paths::to_forward_slashes;
use usta_core::plan::FileOp;
use usta_core::resolver;
use usta_core::snapshot::{ManagedLock, Snapshot};
use usta_ports::clock::Clock;
use usta_ports::fs::{FileSystem, FsError};
use usta_ports::renderer::TemplateRenderer;

use crate::scaffold::plan_executor::sha256_hex;
use crate::scaffold::snapshot::{LOCK_PATH, SNAPSHOT_PATH};
use crate::scaffold::{plan_builder, snapshot, ScaffoldError};

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
    Domain(#[from] usta_core::DomainError),
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
    let mut snapshot = read_snapshot(fs)?;
    let prior_lock = read_lock(fs)?;

    // 2. Resolve the prior feature set against the (possibly newer) template.
    use std::collections::BTreeSet;
    let prior_features: BTreeSet<_> = snapshot.features.iter().cloned().collect();
    let resolved = resolver::resolve(&template.manifest, &prior_features)?;

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
                let rendered: Vec<usta_core::plan::AnchorContribution> = contributions
                    .iter()
                    .map(|c| usta_core::plan::AnchorContribution {
                        marker: c.marker.clone(),
                        content: renderer
                            .render(&c.content, &snapshot.answers)
                            .unwrap_or_else(|_| c.content.clone()),
                    })
                    .collect();
                let injected = usta_core::inject::apply_injections(source, &rendered);
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

    // 6. Persist.
    snapshot.template_version = template.manifest.meta.version.clone();
    snapshot.usta_version = req.usta_version;
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
    format: usta_core::plan::MergeFormat,
    overlay: &serde_json::Value,
) -> Result<Vec<u8>, UpdateError> {
    use usta_core::merge::{canonicalize_keys, deep_merge};

    let mut current: serde_json::Value = if fs.exists(path) {
        let bytes = fs.read(path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| {
            UpdateError::Fs(format!("merge target `{}` not UTF-8: {e}", path.display()))
        })?;
        match format {
            usta_core::plan::MergeFormat::Json => serde_json::from_str(text)
                .map_err(|e| UpdateError::Fs(format!("parse JSON `{}`: {e}", path.display())))?,
            usta_core::plan::MergeFormat::Toml => toml::from_str(text)
                .map_err(|e| UpdateError::Fs(format!("parse TOML `{}`: {e}", path.display())))?,
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    deep_merge(&mut current, overlay);
    canonicalize_keys(&mut current);

    let out = match format {
        usta_core::plan::MergeFormat::Json => {
            let mut s = serde_json::to_string_pretty(&current)
                .map_err(|e| UpdateError::Fs(format!("emit JSON: {e}")))?;
            s.push('\n');
            s
        }
        usta_core::plan::MergeFormat::Toml => {
            let toml_value: toml::Value = serde_json::from_value(current.clone())
                .map_err(|e| UpdateError::Fs(format!("JSON→TOML convert: {e}")))?;
            toml::to_string_pretty(&toml_value)
                .map_err(|e| UpdateError::Fs(format!("emit TOML: {e}")))?
        }
    };
    Ok(out.into_bytes())
}
