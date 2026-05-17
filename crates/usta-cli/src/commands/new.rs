//! `usta new` — scaffold a new project from a template.
//!
//! P1 wiring: validate name → load template from filesystem source → run
//! prompts (or default-through with `--yes`) → resolve features → execute
//! scaffold via `ScaffoldService`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use usta_core::project::ProjectName;
use usta_core::template::{FeatureId, PromptKind, TemplateId};
use usta_ports::prompts::PromptUi;
use usta_ports::template_source::TemplateSource;

use crate::wiring;

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Project name (kebab-case). Prompted if omitted.
    pub name: Option<String>,

    /// Template id, or `gh:org/repo` for a registry template (P5).
    #[arg(long, default_value = "hello-world")]
    pub template: String,

    /// Comma-separated feature ids (skip prompts).
    #[arg(long, value_delimiter = ',')]
    pub features: Vec<String>,

    /// Package manager to invoke for install (P5).
    #[arg(long)]
    pub pm: Option<String>,

    /// Skip `git init` after scaffold. (Hooked up in P5.)
    #[arg(long)]
    pub no_git: bool,

    /// Skip running the package manager's install. (Hooked up in P5.)
    #[arg(long)]
    pub no_install: bool,

    /// Print the plan instead of writing files. Hooked up in P1.j.
    #[arg(long)]
    pub dry_run: bool,

    /// Accept defaults for every prompt.
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Overwrite a non-empty target directory.
    #[arg(long)]
    pub force: bool,

    /// Run lint/typecheck/build after scaffold (P5).
    #[arg(long)]
    pub verify: bool,

    /// Record all answered prompts (incl. project name + features) to a
    /// TOML file. Useful for testing / CI / sharing setup recipes.
    /// Written only on a successful scaffold.
    #[arg(long)]
    pub record: Option<PathBuf>,

    /// Replay prompt answers from a TOML file produced by `--record`.
    /// Skips every prompt; the file's `template` and `features` win over
    /// any conflicting `--template` / `--features` flags.
    #[arg(long, conflicts_with = "record")]
    pub replay: Option<PathBuf>,

    /// Override auto-derived npm scope.
    #[arg(long)]
    pub scope: Option<String>,

    /// Directory containing template definitions. Defaults to `./templates/`
    /// in the current directory; in P5 this is replaced by an embedded +
    /// cache + registry composite.
    #[arg(long, env = "USTA_TEMPLATES_DIR")]
    pub templates_dir: Option<PathBuf>,

    /// Parent directory under which the project will be created. Default:
    /// current working directory. Final project path = `<output>/<name>`.
    /// Both relative and absolute paths are accepted; the directory is
    /// created if missing (unless `--dry-run`).
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
}

pub fn run(args: NewArgs) -> Result<()> {
    // Replay loads the recorded answer file up front; thereafter we never
    // call the prompt UI, just consult the file.
    let replay: Option<RecordedAnswers> = match args.replay.as_ref() {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .with_context(|| format!("reading replay file {}", p.display()))?;
            let r: RecordedAnswers = toml::from_str(&text)
                .with_context(|| format!("parsing replay file {}", p.display()))?;
            Some(r)
        }
        None => None,
    };

    // In replay mode, skip all interactive prompting.
    let non_interactive = args.yes || replay.is_some();
    let prompt_ui = wiring::build_prompt_ui(non_interactive);

    // 1. Resolve project name. Precedence:
    //    positional `--name` > replay file > interactive prompt.
    //    The positional override is intentional: `--replay` captures a
    //    recipe (template + features + custom answers); the project name
    //    is what's typically different per-scaffold.
    let raw_name = if let Some(n) = args.name.clone() {
        n
    } else if let Some(r) = replay.as_ref() {
        r.answers
            .get("project_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("replay file missing `answers.project_name`"))?
    } else {
        prompt_ui
            .text("Project name", None)
            .map_err(|e| anyhow!("prompt failed: {e}"))?
    };
    let project_name = ProjectName::parse(raw_name.clone())
        .map_err(|e| anyhow!("invalid project name `{raw_name}`: {e}"))?;

    // 2. Resolve target directory.
    //    Precedence: --output flag > current working directory.
    //    Final path is always `<base>/<name>`.
    let base_dir = match args.output.as_ref() {
        Some(p) => {
            // Make relative paths relative to cwd, but otherwise leave as-is.
            // Don't canonicalize yet — the dir may not exist.
            if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir().context("getting cwd")?.join(p)
            }
        }
        None => std::env::current_dir().context("getting cwd")?,
    };
    let target = base_dir.join(project_name.as_str());

    if !args.dry_run {
        // Ensure the parent (--output) directory exists. We never create
        // directories outside of `target` other than this single parent.
        if !base_dir.exists() {
            std::fs::create_dir_all(&base_dir)
                .with_context(|| format!("creating output dir {}", base_dir.display()))?;
        } else if !base_dir.is_dir() {
            return Err(anyhow!(
                "--output `{}` exists but is not a directory",
                base_dir.display()
            ));
        }

        if target.exists() && !args.force {
            let is_empty = target
                .read_dir()
                .map(|mut it| it.next().is_none())
                .unwrap_or(false);
            if !is_empty {
                return Err(anyhow!(
                    "target {} exists and is not empty (use --force to overwrite)",
                    target.display()
                ));
            }
        }
        std::fs::create_dir_all(&target)
            .with_context(|| format!("creating {}", target.display()))?;
    }

    // Resolve symlinks etc. so the snapshot/lock + the user's shell prompt
    // see a stable absolute path.
    let target = if !args.dry_run {
        target
            .canonicalize()
            .with_context(|| format!("resolving {}", target.display()))?
    } else {
        target
    };

    // 3. Load the template from a filesystem source.
    //    Replay's template id wins over --template if both set.
    let templates_dir = resolve_templates_dir(args.templates_dir.as_ref())
        .context("locating templates directory")?;
    let source = wiring::build_template_source(templates_dir.clone());
    let template_id_str = replay
        .as_ref()
        .map(|r| r.template.clone())
        .unwrap_or_else(|| args.template.clone());
    let tid = TemplateId(template_id_str.clone());
    let template = source
        .load(&tid)
        .map_err(|e| anyhow!("loading template `{}`: {e}", template_id_str))?;

    // 4. Build the answer context.
    //    Replay reuses the recorded answer map verbatim; otherwise we run
    //    the manifest's prompts.
    let mut answers: BTreeMap<String, serde_json::Value> = if let Some(r) = replay.as_ref() {
        r.answers.clone()
    } else {
        let mut m: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        m.insert(
            "project_name".to_string(),
            serde_json::json!(project_name.as_str()),
        );
        let scope = args
            .scope
            .clone()
            .unwrap_or_else(|| project_name.as_str().to_string());
        m.insert("scope".to_string(), serde_json::json!(scope));

        for prompt in &template.manifest.prompts {
            let answer = run_prompt(prompt, prompt_ui.as_ref())?;
            m.insert(prompt.id.clone(), answer);
        }
        m
    };
    // Always overwrite project_name + scope from the resolved values, so
    // a positional `--name` wins over any value baked into the replay file.
    // This is what makes `--replay` reusable across multiple project names.
    answers.insert(
        "project_name".to_string(),
        serde_json::json!(project_name.as_str()),
    );
    let scope = args.scope.clone().unwrap_or_else(|| {
        // For replay, prefer recorded scope; else derive from name.
        replay
            .as_ref()
            .and_then(|r| r.answers.get("scope"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| project_name.as_str().to_string())
    });
    answers.insert("scope".to_string(), serde_json::json!(scope));

    // 5. Resolve feature selection (replay wins → --features → defaults → prompt).
    let selected: BTreeSet<FeatureId> = if let Some(r) = replay.as_ref() {
        r.features.iter().map(|s| FeatureId(s.clone())).collect()
    } else if !args.features.is_empty() {
        args.features.iter().map(|s| FeatureId(s.clone())).collect()
    } else if args.yes {
        // Take defaults from the manifest.
        template
            .manifest
            .features
            .iter()
            .filter(|f| f.default)
            .map(|f| f.id.clone())
            .collect()
    } else {
        prompt_features(&template, prompt_ui.as_ref())?
    };

    // 6. Dry-run? Print the plan and exit before doing any I/O.
    if args.dry_run {
        let resolved = usta_core::resolver::resolve(&template.manifest, &selected)
            .map_err(|e| anyhow!("resolve failed: {e}"))?;
        let plan = usta_app::scaffold::plan_builder::build_plan(
            &template,
            &resolved,
            &answers,
            target.clone(),
        );
        print_dry_run(&plan, &target, &args.template, &resolved);
        return Ok(());
    }

    // 7. Run the scaffold.
    let svc = wiring::build_scaffold_service(target.clone());
    let outcome = svc
        .run(
            &template,
            usta_app::scaffold::service::ScaffoldRequest {
                root: target.clone(),
                features: selected.clone(),
                answers: answers.clone(),
                force: args.force,
                usta_version: env!("CARGO_PKG_VERSION").to_string(),
            },
        )
        .map_err(|e| anyhow!("scaffold failed: {e}"))?;

    // 7b. Record answers if requested (only on success).
    if let Some(record_path) = args.record.as_ref() {
        let rec = RecordedAnswers {
            usta_version: env!("CARGO_PKG_VERSION").to_string(),
            template: template_id_str.clone(),
            features: outcome
                .resolved_features
                .iter()
                .map(|f| f.0.clone())
                .collect(),
            answers,
        };
        let text = toml::to_string_pretty(&rec).map_err(|e| anyhow!("serialize record: {e}"))?;
        if let Some(parent) = record_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        std::fs::write(record_path, text)
            .with_context(|| format!("writing {}", record_path.display()))?;
    }

    // 7. Report.
    println!(
        "✓ scaffolded `{}` from template `{}` ({} files, features: {})",
        project_name.as_str(),
        args.template,
        outcome.files_written,
        if outcome.resolved_features.is_empty() {
            "none".to_string()
        } else {
            outcome
                .resolved_features
                .iter()
                .map(|f| f.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!("→ next: cd {}", target.display());
    Ok(())
}

/// Print a `--dry-run` summary of what would be written, with one line per
/// `FileOp` annotated by kind:
/// - `+`  Write   (new file)
/// - `~`  Merge   (deep-merge into structured config)
/// - `*`  Inject  (anchor-marker injection)
fn print_dry_run(
    plan: &usta_core::plan::ScaffoldPlan,
    target: &std::path::Path,
    template_id: &str,
    resolved: &[FeatureId],
) {
    use usta_core::plan::FileOp;
    println!(
        "usta new (dry-run): would scaffold {} files at {}",
        plan.ops.len(),
        target.display()
    );
    println!("  template: {}", template_id);
    println!(
        "  features: {}",
        if resolved.is_empty() {
            "(none)".to_string()
        } else {
            resolved
                .iter()
                .map(|f| f.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!();

    let mut writes = 0usize;
    let mut merges = 0usize;
    let mut injects = 0usize;

    for op in &plan.ops {
        match op {
            FileOp::Write { path, contents } => {
                writes += 1;
                println!(
                    "  + {:<60} ({})",
                    path.display(),
                    human_size(contents.len())
                );
            }
            FileOp::Merge { path, format, .. } => {
                merges += 1;
                let fmt = match format {
                    usta_core::plan::MergeFormat::Json => "json",
                    usta_core::plan::MergeFormat::Toml => "toml",
                };
                println!("  ~ {:<60} (deep-merge, {fmt})", path.display());
            }
            FileOp::Inject {
                path,
                contributions,
            } => {
                injects += 1;
                println!(
                    "  * {:<60} (inject {} contribution{})",
                    path.display(),
                    contributions.len(),
                    if contributions.len() == 1 { "" } else { "s" }
                );
            }
        }
    }

    println!();
    println!(
        "  totals: {} write{}, {} merge{}, {} inject{}",
        writes,
        if writes == 1 { "" } else { "s" },
        merges,
        if merges == 1 { "" } else { "s" },
        injects,
        if injects == 1 { "" } else { "s" },
    );
    println!("→ run without --dry-run to apply.");
}

/// Format `n` bytes as `1.2 KiB` / `512 B` / etc. Display only.
fn human_size(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", n as f64 / (1024.0 * 1024.0))
    }
}

/// On-disk shape for `--record` / `--replay` files.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RecordedAnswers {
    /// `usta` CLI version that wrote this file.
    usta_version: String,
    /// Template id used.
    template: String,
    /// Resolved feature ids (post-resolver, includes auto-pulled requires).
    features: Vec<String>,
    /// Answer map used at scaffold time. `project_name` and `scope` are
    /// stored alongside template-prompted answers for portability.
    answers: BTreeMap<String, serde_json::Value>,
}

fn resolve_templates_dir(explicit: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.is_dir() {
            return Err(anyhow!("--templates-dir not a directory: {}", p.display()));
        }
        return Ok(p.clone());
    }
    // Default: look up `./templates/` relative to cwd, then walk upward to the
    // workspace root (containing `Cargo.toml` workspace) — useful in dev.
    let cwd = std::env::current_dir()?;
    for dir in cwd.ancestors() {
        let cand = dir.join("templates");
        if cand.is_dir() {
            return Ok(cand);
        }
    }
    Err(anyhow!(
        "no templates directory found; pass --templates-dir or USTA_TEMPLATES_DIR"
    ))
}

fn run_prompt(
    prompt: &usta_core::template::Prompt,
    ui: &dyn PromptUi,
) -> Result<serde_json::Value> {
    let default_str = prompt.default.as_deref();
    Ok(match prompt.kind {
        PromptKind::Text => {
            let s = ui
                .text(&prompt.question, default_str)
                .map_err(|e| anyhow!("prompt `{}` failed: {e}", prompt.id))?;
            serde_json::json!(s)
        }
        PromptKind::Confirm => {
            let default = matches!(default_str, Some("true") | Some("yes") | Some("y"));
            let b = ui
                .confirm(&prompt.question, default)
                .map_err(|e| anyhow!("prompt `{}` failed: {e}", prompt.id))?;
            serde_json::json!(b)
        }
        PromptKind::Select => {
            let idx = ui
                .select(&prompt.question, &prompt.options)
                .map_err(|e| anyhow!("prompt `{}` failed: {e}", prompt.id))?;
            serde_json::json!(prompt.options.get(idx).cloned().unwrap_or_default())
        }
        PromptKind::Multiselect => {
            let defaults: Vec<bool> = prompt.options.iter().map(|_| false).collect();
            let chosen = ui
                .multiselect(&prompt.question, &prompt.options, &defaults)
                .map_err(|e| anyhow!("prompt `{}` failed: {e}", prompt.id))?;
            let picked: Vec<String> = chosen
                .iter()
                .filter_map(|&i| prompt.options.get(i).cloned())
                .collect();
            serde_json::json!(picked)
        }
    })
}

fn prompt_features(
    template: &usta_core::loaded::LoadedTemplate,
    ui: &dyn PromptUi,
) -> Result<BTreeSet<FeatureId>> {
    if template.manifest.features.is_empty() {
        return Ok(BTreeSet::new());
    }
    let labels: Vec<String> = template
        .manifest
        .features
        .iter()
        .map(|f| format!("{} — {}", f.id.0, f.display_name))
        .collect();
    let defaults: Vec<bool> = template
        .manifest
        .features
        .iter()
        .map(|f| f.default)
        .collect();
    let chosen = ui
        .multiselect("Features", &labels, &defaults)
        .map_err(|e| anyhow!("feature selection failed: {e}"))?;
    Ok(chosen
        .into_iter()
        .filter_map(|i| template.manifest.features.get(i).map(|f| f.id.clone()))
        .collect())
}
