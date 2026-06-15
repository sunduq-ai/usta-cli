//! Package-manager adapters + ecosystem detection for the post-scaffold
//! install step of `usta new`.
//!
//! Detection is split in two so the interesting logic stays pure and
//! unit-testable:
//! - [`classify`] takes the set of manifest paths a project contains and
//!   decides which install commands to run, in which directories. No I/O.
//! - [`detect`] walks a real project tree (bounded — it never descends into
//!   `node_modules`, `.venv`, `target`, `.git`) and feeds [`classify`].
//!
//! The actual install shells out via [`CliPackageManager`], which implements
//! the [`PackageManager`] port. Every install is best-effort: a missing tool
//! is skipped with a note, a failing tool warns — neither aborts the
//! already-completed scaffold.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use walkdir::WalkDir;

use crate::ports::pkg_manager::{PackageManager, PkgError};

/// A package ecosystem we know how to install dependencies for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecosystem {
    /// pnpm workspace (`pnpm install`).
    Pnpm,
    /// npm project (`npm install`).
    Npm,
    /// uv-managed Python project (`uv sync`).
    Uv,
    /// Go module (`go mod download`).
    Go,
}

impl Ecosystem {
    /// The concrete CLI adapter for this ecosystem.
    pub fn manager(self) -> CliPackageManager {
        match self {
            Ecosystem::Pnpm => CliPackageManager::new("pnpm", &["install"]),
            Ecosystem::Npm => CliPackageManager::new("npm", &["install"]),
            Ecosystem::Uv => CliPackageManager::new("uv", &["sync"]),
            Ecosystem::Go => CliPackageManager::new("go", &["mod", "download"]),
        }
    }
}

/// One install to perform: which ecosystem, in which directory (relative to
/// the project root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTarget {
    /// Ecosystem to install with.
    pub ecosystem: Ecosystem,
    /// Directory (relative to the project root) to run the install in.
    pub dir: PathBuf,
}

/// Manifest filenames we key off, relative to the project root.
fn is_under(child: &Path, ancestor: &Path) -> bool {
    // `ancestor` is a strict-or-equal prefix of `child`. Both are relative.
    child == ancestor || child.starts_with(ancestor)
}

/// Decide the install plan from the manifest paths a project contains.
///
/// Pure. `manifests` are paths relative to the project root (e.g.
/// `apps/api/pyproject.toml`). Rules:
/// - **JS, at most one install:** any `pnpm-workspace.yaml` → a single
///   `pnpm install` at its directory (a pnpm workspace install covers every
///   member `package.json`). Otherwise a root `package.json` → `npm install`
///   at the root. Nested `package.json`s without a workspace are left alone —
///   walking arbitrary JS sub-projects is out of scope.
/// - **Python:** each `pyproject.toml` directory gets a `uv sync`, unless it
///   sits under another `pyproject.toml` directory already covered.
/// - **Go:** each `go.mod` directory gets `go mod download`, same nesting
///   rule.
///
/// Targets come back in a stable order (JS, then Python, then Go; each by
/// directory) so output is deterministic.
pub fn classify(manifests: &[PathBuf]) -> Vec<InstallTarget> {
    let names: BTreeSet<PathBuf> = manifests.iter().cloned().collect();
    let mut targets: Vec<InstallTarget> = Vec::new();

    let dir_of = |p: &Path| -> PathBuf { p.parent().unwrap_or(Path::new("")).to_path_buf() };

    // ── JS: at most one install ──────────────────────────────────────────
    let mut pnpm_dirs: Vec<PathBuf> = names
        .iter()
        .filter(|p| {
            p.file_name()
                .map(|n| n == "pnpm-workspace.yaml")
                .unwrap_or(false)
        })
        .map(|p| dir_of(p))
        .collect();
    pnpm_dirs.sort();
    if let Some(root_ws) = pnpm_dirs.first() {
        targets.push(InstallTarget {
            ecosystem: Ecosystem::Pnpm,
            dir: root_ws.clone(),
        });
    } else if names
        .iter()
        .any(|p| p.as_path() == Path::new("package.json"))
    {
        targets.push(InstallTarget {
            ecosystem: Ecosystem::Npm,
            dir: PathBuf::new(),
        });
    }

    // ── Python: one uv sync per top-level pyproject dir ──────────────────
    let mut py_dirs: Vec<PathBuf> = names
        .iter()
        .filter(|p| {
            p.file_name()
                .map(|n| n == "pyproject.toml")
                .unwrap_or(false)
        })
        .map(|p| dir_of(p))
        .collect();
    py_dirs.sort();
    let mut kept_py: Vec<PathBuf> = Vec::new();
    for d in &py_dirs {
        if !kept_py.iter().any(|anc| is_under(d, anc)) {
            kept_py.push(d.clone());
        }
    }
    for d in kept_py {
        targets.push(InstallTarget {
            ecosystem: Ecosystem::Uv,
            dir: d,
        });
    }

    // ── Go: one download per top-level go.mod dir ────────────────────────
    let mut go_dirs: Vec<PathBuf> = names
        .iter()
        .filter(|p| p.file_name().map(|n| n == "go.mod").unwrap_or(false))
        .map(|p| dir_of(p))
        .collect();
    go_dirs.sort();
    let mut kept_go: Vec<PathBuf> = Vec::new();
    for d in &go_dirs {
        if !kept_go.iter().any(|anc| is_under(d, anc)) {
            kept_go.push(d.clone());
        }
    }
    for d in kept_go {
        targets.push(InstallTarget {
            ecosystem: Ecosystem::Go,
            dir: d,
        });
    }

    targets
}

/// Walk a real project tree and return its install plan. Bounded: never
/// descends into dependency/build/VCS directories.
pub fn detect(root: &Path) -> Vec<InstallTarget> {
    const SKIP: &[&str] = &["node_modules", ".venv", "venv", "target", ".git", ".usta"];
    let mut manifests: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            !e.file_type().is_dir()
                || e.file_name()
                    .to_str()
                    .map(|n| !SKIP.contains(&n))
                    .unwrap_or(true)
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let is_manifest = matches!(
            entry.file_name().to_str(),
            Some("pnpm-workspace.yaml" | "package.json" | "pyproject.toml" | "go.mod")
        );
        if is_manifest {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                manifests.push(rel.to_path_buf());
            }
        }
    }
    classify(&manifests)
}

/// A package manager driven by shelling out to a CLI (`pnpm`, `npm`, `uv`,
/// `go`). Holds the tool name and the argv to pass for an install.
#[derive(Debug, Clone)]
pub struct CliPackageManager {
    id: &'static str,
    args: &'static [&'static str],
}

impl CliPackageManager {
    /// Construct from a tool name and its install arguments.
    pub const fn new(id: &'static str, args: &'static [&'static str]) -> Self {
        Self { id, args }
    }
}

impl PackageManager for CliPackageManager {
    fn id(&self) -> &'static str {
        self.id
    }

    fn is_available(&self) -> bool {
        // We only care whether the binary resolves on `$PATH` — so a
        // successful *spawn* is enough, regardless of exit code. This avoids
        // a per-tool flag quirk: `go` rejects `--version` (it wants
        // `go version`) and exits non-zero, which a `status.success()` check
        // would wrongly read as "go is not installed".
        Command::new(self.id).arg("--version").output().is_ok()
    }

    fn install(&self, cwd: &Path) -> Result<(), PkgError> {
        let out = Command::new(self.id)
            .args(self.args)
            .current_dir(cwd)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    PkgError::NotFound(self.id.to_string())
                } else {
                    PkgError::Failed {
                        tool: self.id.to_string(),
                        code: -1,
                        message: e.to_string(),
                    }
                }
            })?;
        if !out.status.success() {
            return Err(PkgError::Failed {
                tool: self.id.to_string(),
                code: out.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn classify_empty_is_empty() {
        assert!(classify(&[]).is_empty());
    }

    #[test]
    fn hello_world_single_npm() {
        // Only a root package.json, no workspace file.
        let t = classify(&[p("package.json")]);
        assert_eq!(
            t,
            vec![InstallTarget {
                ecosystem: Ecosystem::Npm,
                dir: PathBuf::new()
            }]
        );
    }

    #[test]
    fn pnpm_workspace_wins_over_member_package_jsons() {
        // nx-monorepo shape: a workspace file at root plus many member
        // package.jsons → exactly one pnpm install at the root, no npm.
        let t = classify(&[
            p("package.json"),
            p("pnpm-workspace.yaml"),
            p("apps/web/package.json"),
            p("packages/shared/ui/package.json"),
        ]);
        assert_eq!(
            t,
            vec![InstallTarget {
                ecosystem: Ecosystem::Pnpm,
                dir: PathBuf::new()
            }]
        );
    }

    #[test]
    fn python_uv_per_pyproject_dir() {
        let t = classify(&[p("pnpm-workspace.yaml"), p("apps/api/pyproject.toml")]);
        assert_eq!(
            t,
            vec![
                InstallTarget {
                    ecosystem: Ecosystem::Pnpm,
                    dir: PathBuf::new()
                },
                InstallTarget {
                    ecosystem: Ecosystem::Uv,
                    dir: p("apps/api")
                },
            ]
        );
    }

    #[test]
    fn nested_pyproject_is_not_double_installed() {
        // A pyproject under another pyproject dir is covered by the outer one.
        let t = classify(&[p("pyproject.toml"), p("packages/inner/pyproject.toml")]);
        assert_eq!(
            t,
            vec![InstallTarget {
                ecosystem: Ecosystem::Uv,
                dir: PathBuf::new()
            }]
        );
    }

    #[test]
    fn go_modules_detected() {
        let t = classify(&[p("go.mod")]);
        assert_eq!(
            t,
            vec![InstallTarget {
                ecosystem: Ecosystem::Go,
                dir: PathBuf::new()
            }]
        );
    }

    #[test]
    fn detect_walks_a_real_tree_and_skips_node_modules() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::write(root.join("package.json"), "{}").unwrap();
        std::fs::write(root.join("pnpm-workspace.yaml"), "packages: []").unwrap();
        std::fs::create_dir_all(root.join("apps/api")).unwrap();
        std::fs::write(root.join("apps/api/pyproject.toml"), "").unwrap();
        // Noise that must be ignored:
        std::fs::create_dir_all(root.join("node_modules/foo")).unwrap();
        std::fs::write(root.join("node_modules/foo/package.json"), "{}").unwrap();

        let t = detect(root);
        assert_eq!(
            t,
            vec![
                InstallTarget {
                    ecosystem: Ecosystem::Pnpm,
                    dir: PathBuf::new()
                },
                InstallTarget {
                    ecosystem: Ecosystem::Uv,
                    dir: p("apps/api")
                },
            ]
        );
    }
}
