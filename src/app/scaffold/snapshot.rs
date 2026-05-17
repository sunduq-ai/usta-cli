//! Snapshot writer.
//!
//! Persists `.usta/snapshot.toml` and `.usta/managed.lock` into the
//! generated project. Pure modulo the FS port.

use std::path::PathBuf;

use crate::core::snapshot::{ManagedLock, Snapshot};
use crate::ports::fs::FileSystem;

use super::ScaffoldError;

/// Where the snapshot files live, relative to the project root.
pub const SNAPSHOT_PATH: &str = ".usta/snapshot.toml";
/// Where the lock file lives, relative to the project root.
pub const LOCK_PATH: &str = ".usta/managed.lock";

/// Write the snapshot + lock into the project via `fs`.
pub fn write_snapshot<F: FileSystem>(
    fs: &F,
    snapshot: &Snapshot,
    lock: &ManagedLock,
) -> Result<(), ScaffoldError> {
    let snapshot_text = toml::to_string_pretty(snapshot)
        .map_err(|e| ScaffoldError::Render(format!("serialize snapshot: {e}")))?;
    fs.write(
        &PathBuf::from(SNAPSHOT_PATH),
        snapshot_text.as_bytes(),
        true,
    )?;
    fs.write(&PathBuf::from(LOCK_PATH), lock.to_text().as_bytes(), true)?;
    Ok(())
}
