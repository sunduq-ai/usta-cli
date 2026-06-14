//! `usta add <feature>` use case — apply a feature to an already-scaffolded
//! project.
//!
//! Algorithm:
//!
//! 1. Read `.usta/snapshot.toml` to learn the template id/version and the
//!    answers used at scaffold time.
//! 2. Read `.usta/managed.lock` to know which files the template owns.
//! 3. Load the template from a [`TemplateSource`].
//! 4. Resolve the augmented feature set (prior ∪ new) through the
//!    [`resolver`] so transitive `requires` are pulled in.
//! 5. Build a plan with **only the new features**' contributions.
//! 6. Execute:
//!    - `Write` ops: refuse to overwrite existing managed files (they
//!      would already have been written by an earlier feature).
//!    - `Merge` ops: re-apply (deep-merge is idempotent for content that's
//!      already merged).
//!    - `Inject` ops: re-apply via [`apply_injections`]. If the target's
//!      marker is no longer present in the file, surface a clear error and
//!      point the user at `usta update` — adding contributions to an
//!      anchor that was already finalized requires the 3-way merge
//!      machinery (`usta update`).
//! 7. Update snapshot (features = resolved order; created_at unchanged but
//!    add a new event timestamp) and lock (merge in new digests).
//!
//! [`TemplateSource`]: crate::ports::template_source::TemplateSource
//! [`resolver`]: crate::core::resolver
//! [`apply_injections`]: crate::core::inject::apply_injections

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::core::loaded::LoadedTemplate;
use crate::core::plan::FileOp;
use crate::core::resolver;
use crate::core::snapshot::{ManagedLock, Snapshot};
use crate::core::template::FeatureId;
use crate::ports::clock::Clock;
use crate::ports::fs::{FileSystem, FsError};
use crate::ports::renderer::TemplateRenderer;
use thiserror::Error;

use super::scaffold::plan_executor::sha256_hex;
use super::scaffold::snapshot::{LOCK_PATH, SNAPSHOT_PATH};
use super::scaffold::{plan_builder, snapshot, ScaffoldError};

/// Errors raised by [`add`].
#[derive(Debug, Error)]
pub enum AddError {
    /// The project is missing `.usta/snapshot.toml` (not a usta project).
    #[error("no `.usta/snapshot.toml` found at project root (was this scaffolded by usta?)")]
    NoSnapshot,
    /// Lock file missing or malformed.
    #[error("invalid managed.lock: {0}")]
    InvalidLock(String),
    /// Snapshot file malformed.
    #[error("invalid snapshot.toml: {0}")]
    InvalidSnapshot(String),
    /// One of the requested feature ids is unknown to the template.
    #[error("unknown feature: {0}")]
    UnknownFeature(String),
    /// The feature is already enabled in the project.
    #[error("feature `{0}` is already applied to this project")]
    AlreadyApplied(String),
    /// The new feature wants to inject into a marker that no longer exists.
    /// Direct the user at `usta update`.
    #[error("feature `{feature}` wants to inject into `{path}` but marker `{marker}` is no longer present (use `usta update` to re-render)")]
    AnchorMarkerMissing {
        /// Feature being added.
        feature: String,
        /// Target file the inject was targeting.
        path: PathBuf,
        /// Marker name.
        marker: String,
    },
    /// Adding the feature would overwrite a file that already exists.
    /// User should either back up the file or use `--force`.
    #[error(
        "cowardly refusing to overwrite existing file `{0}` (rerun with --force to overwrite)"
    )]
    WouldOverwrite(PathBuf),
    /// Underlying scaffold engine error.
    #[error(transparent)]
    Scaffold(#[from] ScaffoldError),
    /// Domain-rule violation surfaced by the resolver.
    #[error(transparent)]
    Domain(#[from] crate::core::DomainError),
    /// FS port returned an error.
    #[error("filesystem: {0}")]
    Fs(String),
}

impl From<FsError> for AddError {
    fn from(e: FsError) -> Self {
        AddError::Fs(e.to_string())
    }
}

/// Inputs to a single add run.
#[derive(Debug)]
pub struct AddRequest {
    /// New feature ids to add.
    pub new_features: BTreeSet<FeatureId>,
    /// `usta` CLI version recording the new state (snapshot bookkeeping).
    pub usta_version: String,
    /// Whether to overwrite existing managed files when their write op
    /// collides with current content.
    pub force: bool,
}

/// Result of an add run.
#[derive(Debug, Clone)]
pub struct AddOutcome {
    /// All features (prior + newly added, in resolution order).
    pub resolved_features: Vec<FeatureId>,
    /// Just the newly added feature ids (preserving resolver order).
    pub newly_added: Vec<FeatureId>,
    /// Files written (new + merged + re-injected).
    pub files_written: usize,
}

/// Run the add use case.
pub fn add<F, R, C>(
    fs: &F,
    renderer: &R,
    clock: &C,
    template: &LoadedTemplate,
    req: AddRequest,
) -> Result<AddOutcome, AddError>
where
    F: FileSystem,
    R: TemplateRenderer,
    C: Clock,
{
    // 1. Read existing snapshot + lock.
    let mut prior_snapshot = read_snapshot(fs)?;
    let mut prior_lock = read_lock(fs)?;

    // 2. Validate new features exist + aren't already applied.
    let prior_features: BTreeSet<FeatureId> = prior_snapshot.features.iter().cloned().collect();
    let known_ids: BTreeSet<FeatureId> = template
        .manifest
        .features
        .iter()
        .map(|f| f.id.clone())
        .collect();
    for f in &req.new_features {
        if !known_ids.contains(f) {
            return Err(AddError::UnknownFeature(f.0.clone()));
        }
        if prior_features.contains(f) {
            return Err(AddError::AlreadyApplied(f.0.clone()));
        }
    }

    // 3. Resolve the augmented set, capturing what's actually new.
    let augmented: BTreeSet<FeatureId> = prior_features.union(&req.new_features).cloned().collect();
    let resolved = resolver::resolve(&template.manifest, &augmented)?;
    let newly_added: Vec<FeatureId> = resolved
        .iter()
        .filter(|f| !prior_features.contains(f))
        .cloned()
        .collect();

    // 4. Build a plan for ONLY the newly added features (excluding base —
    //    base files are already on disk from the original scaffold).
    let project_root = PathBuf::new(); // FS adapter is jailed at root; ops use relative paths.
    let plan = plan_builder::build_features_only_plan(
        template,
        &newly_added,
        &prior_snapshot.answers,
        project_root.clone(),
    );

    // 5. Execute the plan, with add-specific guards.
    let mut new_lock_entries: BTreeMap<PathBuf, String> = BTreeMap::new();
    for op in &plan.ops {
        match op {
            FileOp::Write { path, contents } => {
                let bytes =
                    render_for_write(renderer, template, &prior_snapshot.answers, path, contents)?;
                if fs.exists(path) && !req.force {
                    return Err(AddError::WouldOverwrite(path.clone()));
                }
                fs.write(path, &bytes, true)?;
                new_lock_entries.insert(path.clone(), sha256_hex(&bytes));
            }
            FileOp::Merge {
                path,
                format,
                value,
            } => {
                let bytes = apply_merge_via_engine(fs, path, *format, value)?;
                fs.write(path, &bytes, true)?;
                new_lock_entries.insert(path.clone(), sha256_hex(&bytes));
            }
            FileOp::Inject {
                path,
                contributions,
            } => {
                // Read current. Render each contribution. Apply only those
                // whose marker still exists in the file. Error if ANY
                // contribution's marker is missing — that means this
                // feature can't be cleanly applied post-hoc.
                let existing = fs.read(path).map_err(|e| {
                    AddError::Fs(format!("inject target `{}` not found: {e}", path.display()))
                })?;
                let source = std::str::from_utf8(&existing).map_err(|e| {
                    AddError::Fs(format!("inject target `{}` not UTF-8: {e}", path.display()))
                })?;

                // Gather rendered contributions, while validating markers exist.
                let mut rendered: Vec<crate::core::plan::AnchorContribution> = Vec::new();
                for c in contributions {
                    let rendered_content = renderer
                        .render(&c.content, &prior_snapshot.answers)
                        .unwrap_or_else(|_| c.content.clone());
                    let marker_present = source
                        .lines()
                        .any(|line| super::__has_marker(line, &c.marker));
                    if !marker_present {
                        // Pinpoint which feature owns this missing marker.
                        let owning_feature = newly_added
                            .iter()
                            .find(|fid| {
                                template
                                    .feature_injections
                                    .get(fid)
                                    .map(|injs| {
                                        injs.iter().any(|inj| {
                                            inj.target == *path
                                                && inj
                                                    .contributions
                                                    .iter()
                                                    .any(|x| x.marker == c.marker)
                                        })
                                    })
                                    .unwrap_or(false)
                            })
                            .map(|f| f.0.clone())
                            .unwrap_or_else(|| "?".into());
                        return Err(AddError::AnchorMarkerMissing {
                            feature: owning_feature,
                            path: path.clone(),
                            marker: c.marker.clone(),
                        });
                    }
                    rendered.push(crate::core::plan::AnchorContribution {
                        marker: c.marker.clone(),
                        content: rendered_content,
                    });
                }

                let injected = crate::core::inject::apply_injections(source, &rendered);
                let bytes = injected.into_bytes();
                fs.write(path, &bytes, true)?;
                new_lock_entries.insert(path.clone(), sha256_hex(&bytes));
            }
        }
    }

    let files_written = new_lock_entries.len();

    // 6. Persist updated snapshot + lock.
    prior_snapshot.features = resolved.clone();
    prior_snapshot.usta_version = req.usta_version;
    prior_snapshot.created_at = clock.now_rfc3339();
    for (path, hash) in new_lock_entries {
        prior_lock.files.insert(path, hash);
    }
    snapshot::write_snapshot(fs, &prior_snapshot, &prior_lock).map_err(AddError::Scaffold)?;

    Ok(AddOutcome {
        resolved_features: resolved,
        newly_added,
        files_written,
    })
}

fn read_snapshot<F: FileSystem>(fs: &F) -> Result<Snapshot, AddError> {
    let p = Path::new(SNAPSHOT_PATH);
    if !fs.exists(p) {
        return Err(AddError::NoSnapshot);
    }
    let bytes = fs.read(p)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AddError::InvalidSnapshot(format!("not UTF-8: {e}")))?;
    let snap: Snapshot =
        toml::from_str(text).map_err(|e| AddError::InvalidSnapshot(e.to_string()))?;
    Ok(snap)
}

fn read_lock<F: FileSystem>(fs: &F) -> Result<ManagedLock, AddError> {
    let p = Path::new(LOCK_PATH);
    if !fs.exists(p) {
        return Ok(ManagedLock::default());
    }
    let bytes = fs.read(p)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AddError::InvalidLock(format!("not UTF-8: {e}")))?;
    ManagedLock::from_text(text).map_err(|e| AddError::InvalidLock(e.to_string()))
}

fn render_for_write<R: TemplateRenderer>(
    renderer: &R,
    template: &LoadedTemplate,
    answers: &BTreeMap<String, serde_json::Value>,
    path: &Path,
    contents: &[u8],
) -> Result<Vec<u8>, AddError> {
    // Decide whether this path was a Render template at load time.
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
            .map_err(|e| AddError::Fs(format!("template not UTF-8: {e}")))?;
        let rendered = renderer
            .render(source, answers)
            .map_err(|e| AddError::Fs(format!("render: {e}")))?;
        Ok(rendered.into_bytes())
    } else {
        Ok(contents.to_vec())
    }
}

fn apply_merge_via_engine<F: FileSystem>(
    fs: &F,
    path: &Path,
    format: crate::core::plan::MergeFormat,
    overlay: &serde_json::Value,
) -> Result<Vec<u8>, AddError> {
    use crate::core::merge::{canonicalize_keys, deep_merge};

    let mut current: serde_json::Value = if fs.exists(path) {
        let bytes = fs.read(path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| {
            AddError::Fs(format!("merge target `{}` not UTF-8: {e}", path.display()))
        })?;
        match format {
            crate::core::plan::MergeFormat::Json => serde_json::from_str(text)
                .map_err(|e| AddError::Fs(format!("parse JSON `{}`: {e}", path.display())))?,
            crate::core::plan::MergeFormat::Toml => toml::from_str(text)
                .map_err(|e| AddError::Fs(format!("parse TOML `{}`: {e}", path.display())))?,
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    deep_merge(&mut current, overlay);
    canonicalize_keys(&mut current);

    let out = match format {
        crate::core::plan::MergeFormat::Json => {
            let mut s = serde_json::to_string_pretty(&current)
                .map_err(|e| AddError::Fs(format!("emit JSON: {e}")))?;
            s.push('\n');
            s
        }
        crate::core::plan::MergeFormat::Toml => {
            let toml_value: toml::Value = serde_json::from_value(current.clone())
                .map_err(|e| AddError::Fs(format!("JSON→TOML convert: {e}")))?;
            toml::to_string_pretty(&toml_value)
                .map_err(|e| AddError::Fs(format!("emit TOML: {e}")))?
        }
    };
    Ok(out.into_bytes())
}
