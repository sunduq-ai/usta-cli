//! `usta add <feature>` use case — apply a feature to an already-scaffolded
//! project.
//!
//! `add` re-renders the project from the template for the **augmented**
//! feature set (existing ∪ requested) and 3-way-merges the result against
//! the live tree, exactly like `usta update` but with extra features. This
//! is what lets injection-based features be added post-hoc without relying
//! on anchor markers still being present in the user's source — `new`
//! strips all markers, so `add` re-derives the file from the template
//! rather than editing live markers.
//!
//! Algorithm:
//! 1. Read `.usta/snapshot.toml` + `.usta/managed.lock`.
//! 2. Validate the requested features exist and aren't already applied.
//! 3. Resolve the augmented set through the [`resolver`] (pulls in
//!    transitive `requires`).
//! 4. Delegate to [`crate::app::update::regenerate`], which re-renders,
//!    3-way-merges, strips residual markers, and persists snapshot + lock.
//!
//! [`resolver`]: crate::core::resolver

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::core::loaded::LoadedTemplate;
use crate::core::resolver;
use crate::core::snapshot::{ManagedLock, Snapshot};
use crate::core::template::FeatureId;
use crate::ports::clock::Clock;
use crate::ports::fs::{FileSystem, FsError};
use crate::ports::renderer::TemplateRenderer;
use thiserror::Error;

use super::scaffold::snapshot::{LOCK_PATH, SNAPSHOT_PATH};
use super::update::{self, UpdateError};

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
    /// Domain-rule violation surfaced by the resolver.
    #[error(transparent)]
    Domain(#[from] crate::core::DomainError),
    /// Error from the shared regeneration engine.
    #[error(transparent)]
    Regenerate(#[from] UpdateError),
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
}

/// Result of an add run.
#[derive(Debug, Clone)]
pub struct AddOutcome {
    /// All features (prior + newly added, in resolution order).
    pub resolved_features: Vec<FeatureId>,
    /// Just the newly added feature ids (preserving resolver order).
    pub newly_added: Vec<FeatureId>,
    /// Files written (added + overwritten).
    pub files_written: usize,
    /// Files where the user's local edits conflict with the re-render. The
    /// proposed content is at `.usta/proposed/<path>` (same as `update`).
    pub conflicts: Vec<PathBuf>,
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
    let snapshot = read_snapshot(fs)?;
    let prior_lock = read_lock(fs)?;

    // 2. Validate new features exist + aren't already applied.
    let prior_features: BTreeSet<FeatureId> = snapshot.features.iter().cloned().collect();
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

    // 4. Re-render the project for the augmented feature set. This re-derives
    //    injection-based files from the template (markers and all), applies
    //    every resolved feature's contributions, 3-way-merges against the
    //    live tree, strips residual markers, and persists snapshot + lock.
    let outcome = update::regenerate(
        fs,
        renderer,
        clock,
        template,
        resolved.clone(),
        snapshot,
        prior_lock,
        req.usta_version,
    )?;

    Ok(AddOutcome {
        resolved_features: resolved,
        newly_added,
        files_written: outcome.added.len() + outcome.overwritten.len(),
        conflicts: outcome.conflicts,
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
