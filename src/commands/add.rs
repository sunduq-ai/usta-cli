//! `usta add <feature>` — apply a feature to an already-scaffolded project.
//!
//! `add` re-renders the project from the template for the augmented feature
//! set and 3-way-merges against the live tree (see the `add` use case in
//! `crate::app::add`). Injection-based features work post-hoc without
//! relying on anchor markers surviving in the user's source. If a managed
//! file was edited locally, the re-render is written to `.usta/proposed/`
//! and reported as a conflict (exit 40), same as `usta update`.

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

    // Resolve templates dir (built-in templates are the fallback).
    let templates_dir =
        crate::wiring::resolve_templates_dir(args.templates_dir.as_deref(), &project_root)
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

    // Validate against the template so a typo gets a "did you mean?" hint.
    let known: Vec<String> = template
        .manifest
        .features
        .iter()
        .map(|f| f.id.0.clone())
        .collect();
    for f in &new_features {
        if !known.contains(&f.0) {
            let hint = crate::commands::suggestion_hint(&f.0, &known);
            return Err(anyhow!("unknown feature `{}`{hint}", f.0));
        }
    }

    let outcome = add::add(
        &fs,
        &renderer,
        &clock,
        &template,
        add::AddRequest {
            new_features,
            usta_version: env!("CARGO_PKG_VERSION").to_string(),
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

    // If the re-render conflicted with the user's local edits, the proposed
    // content was written under `.usta/proposed/`. Surface it the same way
    // `usta update` does (exit 40) so scripts can react.
    if !outcome.conflicts.is_empty() {
        for p in &outcome.conflicts {
            println!(
                "  ! conflict    {} (proposed at .usta/proposed/{})",
                p.display(),
                p.display()
            );
        }
        println!(
            "→ {} conflict(s); inspect `.usta/proposed/<path>` and merge manually.",
            outcome.conflicts.len()
        );
        std::process::exit(40);
    }
    Ok(())
}
