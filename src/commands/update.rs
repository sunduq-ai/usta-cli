//! `usta update` — re-render the template against stored answers and
//! 3-way merge the result with the project.

use std::path::PathBuf;

use crate::adapters::clock::SystemClock;
use crate::adapters::fs::LocalFs;
use crate::adapters::renderer::MinijinjaRenderer;
use crate::adapters::templates::filesystem_source::FilesystemTemplateSource;
use crate::app::update;
use crate::core::snapshot::Snapshot;
use crate::core::template::TemplateId;
use crate::ports::template_source::TemplateSource;
use anyhow::{anyhow, Context, Result};
use clap::Args;

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Project root (defaults to current directory).
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Templates directory; falls back to walking up from cwd.
    #[arg(long, env = "USTA_TEMPLATES_DIR")]
    pub templates_dir: Option<PathBuf>,

    /// (P5) Pin a target template version. Currently uses the templates
    /// dir's on-disk version.
    #[arg(long)]
    pub to: Option<String>,

    /// (P5) Pause on every conflict to prompt for resolution.
    #[arg(long)]
    pub interactive: bool,

    /// (P5) Restore pre-update state from `.usta/snapshot.toml`.
    #[arg(long, conflicts_with = "interactive")]
    pub abort: bool,
}

pub fn run(args: UpdateArgs) -> Result<()> {
    if args.abort {
        return Err(anyhow!(
            "`--abort` lands in P5 with full snapshot history; use git to revert for now"
        ));
    }
    if args.interactive {
        eprintln!("usta: --interactive lands in P5; running in non-interactive mode.");
    }
    if let Some(_v) = args.to.as_deref() {
        eprintln!("usta: --to lands in P5 with the registry; using the on-disk template version.");
    }

    let project_root = args
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
        .canonicalize()
        .context("resolving project root")?;
    if !project_root.is_dir() {
        return Err(anyhow!(
            "project root not a directory: {}",
            project_root.display()
        ));
    }

    // Read snapshot to learn template id.
    let snapshot_path = project_root.join(".usta/snapshot.toml");
    if !snapshot_path.is_file() {
        return Err(anyhow!(
            "no `.usta/snapshot.toml` at {} (was this project scaffolded by usta?)",
            project_root.display()
        ));
    }
    let snap_text = std::fs::read_to_string(&snapshot_path).context("read snapshot")?;
    let snapshot: Snapshot = toml::from_str(&snap_text).context("parse snapshot")?;

    let templates_dir = resolve_templates_dir(args.templates_dir.as_ref(), &project_root)
        .context("locating templates directory")?;
    let source = FilesystemTemplateSource::new(templates_dir.clone());
    let template = source
        .load(&TemplateId(snapshot.template_id.0.clone()))
        .map_err(|e| anyhow!("loading template `{}`: {e}", snapshot.template_id.0))?;

    let fs = LocalFs::new(&project_root);
    let renderer = MinijinjaRenderer::new();
    let clock = SystemClock::new();

    let outcome = update::update(
        &fs,
        &renderer,
        &clock,
        &template,
        update::UpdateRequest {
            usta_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    )
    .map_err(|e| anyhow!("update failed: {e}"))?;

    println!(
        "✓ update inspected {} files: {} added, {} overwritten, {} unchanged, {} conflicts, {} orphaned",
        outcome.total_inspected(),
        outcome.added.len(),
        outcome.overwritten.len(),
        outcome.unchanged.len(),
        outcome.conflicts.len(),
        outcome.orphaned.len()
    );
    for p in &outcome.added {
        println!("  + added       {}", p.display());
    }
    for p in &outcome.conflicts {
        println!(
            "  ! conflict    {} (proposed at .usta/proposed/{})",
            p.display(),
            p.display()
        );
    }
    for p in &outcome.orphaned {
        println!("  ~ orphaned    {} (no longer in template)", p.display());
    }
    if !outcome.is_clean() {
        println!(
            "→ {} conflict(s); inspect `.usta/proposed/<path>` and merge manually.",
            outcome.conflicts.len()
        );
        std::process::exit(40); // documented exit code
    }
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
