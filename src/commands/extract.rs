//! `usta extract` — synthesize a template from an existing repository.
//!
//! Deterministic by default: same repo + same `.usta-extract.toml` + same
//! `usta` version → identical output. No LLM calls, ever.

use std::path::PathBuf;

use crate::adapters::fs::LocalFs;
use crate::adapters::scanner::ignore_scanner::IgnoreScanner;
use crate::app::extract::{ExtractOutcome, ExtractService};
use crate::core::extract::ExtractConfig;
use anyhow::{anyhow, Context, Result};
use clap::Args;

#[derive(Debug, Args)]
pub struct ExtractArgs {
    /// Path to a local repository to synthesize a template from.
    pub repo: PathBuf,

    /// Parent directory under which the synthesized template will be
    /// written (final path: `<out>/<template-id>/`).
    #[arg(long, default_value = "./templates")]
    pub out: PathBuf,

    /// Override the synthesized template id (defaults to repo dir name).
    #[arg(long)]
    pub name: Option<String>,

    /// Path to a `.usta-extract.toml` config (defaults to looking in the
    /// repo root).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Overwrite an existing `<out>/<template-id>/` directory.
    #[arg(long)]
    pub force: bool,
}

pub fn run(args: ExtractArgs) -> Result<()> {
    // 1. Resolve repo path.
    let repo_root = args
        .repo
        .canonicalize()
        .with_context(|| format!("resolving repo path {}", args.repo.display()))?;
    if !repo_root.is_dir() {
        return Err(anyhow!(
            "repo path is not a directory: {}",
            repo_root.display()
        ));
    }

    // 2. Load config: explicit `--config` > `<repo>/.usta-extract.toml` > default.
    let mut config = load_config(&repo_root, args.config.as_deref())?;

    // 3. Apply CLI overrides.
    if let Some(name) = args.name.clone() {
        config.template_id = Some(name);
    }
    if config.template_id.is_none() {
        // Default: name the template after the repo's directory.
        if let Some(name) = repo_root.file_name().and_then(|n| n.to_str()) {
            config.template_id = Some(name.to_string());
        } else {
            config.template_id = Some("extracted".to_string());
        }
    }

    // 4. Resolve output directory.
    let out_dir = args
        .out
        .canonicalize()
        .or_else(|_| {
            std::fs::create_dir_all(&args.out)?;
            args.out.canonicalize()
        })
        .with_context(|| format!("preparing output dir {}", args.out.display()))?;

    // 5. Wire and run.
    let scanner = IgnoreScanner::new();
    let out_fs = LocalFs::new(&out_dir);
    let service = ExtractService::new(scanner, out_fs, |p| std::fs::read(p));

    let outcome = service
        .run(&repo_root, &config, args.force)
        .map_err(|e| anyhow!("extract failed: {e}"))?;

    report_outcome(&out_dir, &outcome);
    Ok(())
}

fn load_config(
    repo_root: &std::path::Path,
    explicit: Option<&std::path::Path>,
) -> Result<ExtractConfig> {
    let candidate = explicit.map(std::path::PathBuf::from).or_else(|| {
        let p = repo_root.join(".usta-extract.toml");
        p.is_file().then_some(p)
    });

    match candidate {
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let cfg: ExtractConfig =
                toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
            Ok(cfg)
        }
        None => Ok(ExtractConfig::default()),
    }
}

fn report_outcome(out_dir: &std::path::Path, outcome: &ExtractOutcome) {
    println!(
        "✓ extracted template `{}`: {} scanned, {} dropped, {} written",
        outcome.template_id, outcome.scanned, outcome.dropped, outcome.written
    );
    if !outcome.features.is_empty() {
        println!("  features: {}", outcome.features.join(", "));
    }
    println!(
        "→ wrote to {}",
        out_dir.join(&outcome.template_id).display()
    );
    println!(
        "→ scaffold from it: usta new my-app --template {} --templates-dir {}",
        outcome.template_id,
        out_dir.display()
    );
}
