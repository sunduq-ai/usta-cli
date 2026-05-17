//! `usta add <feature>` — apply a feature to an already-scaffolded project.
//!
//! P4.b limitation: features whose contributions inject into anchor markers
//! that have already been finalized (markers stripped from the file) cannot
//! be added post-hoc. Such cases surface as an `AnchorMarkerMissing` error
//! with a pointer to `usta update` (P4.c).

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::adapters::clock::SystemClock;
use crate::adapters::fs::LocalFs;
use crate::adapters::renderer::MinijinjaRenderer;
use crate::adapters::templates::filesystem_source::FilesystemTemplateSource;
use crate::app::add;
use crate::core::snapshot::Snapshot;
use crate::core::template::{FeatureId, TemplateId};
use crate::ports::template_source::TemplateSource;
use anyhow::{anyhow, Context, Result};
use clap::Args;

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Feature ids to add (comma-separated allowed via positional list).
    #[arg(required = true, num_args = 1..)]
    pub features: Vec<String>,

    /// Print the plan instead of writing files. (Hooked up in P1.j.)
    #[arg(long)]
    pub dry_run: bool,

    /// Overwrite existing managed files when contributions collide.
    #[arg(long)]
    pub force: bool,

    /// Project root (defaults to current directory).
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Templates directory; falls back to walking up from cwd.
    #[arg(long, env = "USTA_TEMPLATES_DIR")]
    pub templates_dir: Option<PathBuf>,
}

pub fn run(args: AddArgs) -> Result<()> {
    let project_root = args
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
        .canonicalize()
        .context("resolving project root")?;
    if !project_root.is_dir() {
        return Err(anyhow!(
            "project root is not a directory: {}",
            project_root.display()
        ));
    }

    // Read snapshot directly via std::fs to learn the template id (the FS
    // adapter would also work but this is read-only and avoids creating
    // the LocalFs jail before we know what we need).
    let snapshot_path = project_root.join(".usta/snapshot.toml");
    if !snapshot_path.is_file() {
        return Err(anyhow!(
            "no `.usta/snapshot.toml` at {} (was this project scaffolded by usta?)",
            project_root.display()
        ));
    }
    let snap_text = std::fs::read_to_string(&snapshot_path).context("read snapshot")?;
    let snapshot: Snapshot = toml::from_str(&snap_text).context("parse snapshot")?;

    // Resolve templates dir.
    let templates_dir = resolve_templates_dir(args.templates_dir.as_ref(), &project_root)
        .context("locating templates directory")?;
    let source = FilesystemTemplateSource::new(templates_dir.clone());
    let template = source
        .load(&TemplateId(snapshot.template_id.0.clone()))
        .map_err(|e| anyhow!("loading template `{}`: {e}", snapshot.template_id.0))?;

    // Wire engines.
    let fs = LocalFs::new(&project_root);
    let renderer = MinijinjaRenderer::new();
    let clock = SystemClock::new();

    let new_features: BTreeSet<FeatureId> =
        args.features.iter().map(|s| FeatureId(s.clone())).collect();

    let outcome = add::add(
        &fs,
        &renderer,
        &clock,
        &template,
        add::AddRequest {
            new_features,
            usta_version: env!("CARGO_PKG_VERSION").to_string(),
            force: args.force,
        },
    )
    .map_err(|e| anyhow!("add failed: {e}"))?;

    println!(
        "✓ added {} feature(s): {} ({} files written)",
        outcome.newly_added.len(),
        outcome
            .newly_added
            .iter()
            .map(|f| f.0.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        outcome.files_written
    );
    println!(
        "→ effective feature set: {}",
        outcome
            .resolved_features
            .iter()
            .map(|f| f.0.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

fn resolve_templates_dir(
    explicit: Option<&PathBuf>,
    project_root: &std::path::Path,
) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.is_dir() {
            return Err(anyhow!("--templates-dir not a directory: {}", p.display()));
        }
        return Ok(p.clone());
    }
    // Walk upward looking for a `templates/` directory.
    for dir in project_root.ancestors() {
        let cand = dir.join("templates");
        if cand.is_dir() {
            return Ok(cand);
        }
    }
    Err(anyhow!(
        "no templates directory found; pass --templates-dir or USTA_TEMPLATES_DIR"
    ))
}
