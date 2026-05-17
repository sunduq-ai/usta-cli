//! `usta list` — discover templates and inspect their features.
//!
//! Two subcommands:
//! - `usta list templates` — every template id discoverable from the
//!   active templates directory, with display names + versions.
//! - `usta list features --template <id>` — feature inventory for a
//!   given template.
//!
//! Both support `--json` for scripting.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use usta_adapters::templates::filesystem_source::FilesystemTemplateSource;
use usta_core::template::TemplateId;
use usta_ports::template_source::TemplateSource;

#[derive(Debug, Subcommand)]
pub enum ListCmd {
    /// List installed templates.
    Templates {
        /// Emit JSON.
        #[arg(long)]
        json: bool,
        /// Templates directory (default: walks up from cwd).
        #[arg(long, env = "USTA_TEMPLATES_DIR")]
        templates_dir: Option<PathBuf>,
    },
    /// List features for a given template.
    Features {
        /// Template id.
        #[arg(long)]
        template: String,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
        /// Templates directory (default: walks up from cwd).
        #[arg(long, env = "USTA_TEMPLATES_DIR")]
        templates_dir: Option<PathBuf>,
    },
}

pub fn run(cmd: ListCmd) -> Result<()> {
    match cmd {
        ListCmd::Templates {
            json,
            templates_dir,
        } => list_templates(json, templates_dir),
        ListCmd::Features {
            template,
            json,
            templates_dir,
        } => list_features(&template, json, templates_dir),
    }
}

fn list_templates(json: bool, dir: Option<PathBuf>) -> Result<()> {
    let dir = resolve_dir(dir)?;
    let source = FilesystemTemplateSource::new(&dir);
    let ids = source.list_ids();

    if ids.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("(no templates in {})", dir.display());
        }
        return Ok(());
    }

    // Load each manifest so we can show the display name + version.
    let mut entries: Vec<TemplateEntry> = Vec::with_capacity(ids.len());
    for id in ids {
        let loaded = source
            .load(&id)
            .map_err(|e| anyhow!("loading `{}`: {e}", id.0))?;
        entries.push(TemplateEntry {
            id: id.0.clone(),
            display_name: loaded.manifest.display_name().to_string(),
            version: loaded.manifest.meta.version.to_string(),
            stacks: loaded.manifest.meta.stacks.clone(),
            feature_count: loaded.manifest.features.len(),
        });
    }

    if json {
        let payload = serde_json::json!(entries
            .iter()
            .map(|e| serde_json::json!({
                "id": e.id,
                "display_name": e.display_name,
                "version": e.version,
                "stacks": e.stacks,
                "features": e.feature_count,
            }))
            .collect::<Vec<_>>());
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("Templates in {}:", dir.display());
        for e in &entries {
            let stacks = if e.stacks.is_empty() {
                String::new()
            } else {
                format!("  [{}]", e.stacks.join(", "))
            };
            println!(
                "  {:<24} {} (v{}, {} feature{}){}",
                e.id,
                e.display_name,
                e.version,
                e.feature_count,
                if e.feature_count == 1 { "" } else { "s" },
                stacks
            );
        }
    }
    Ok(())
}

fn list_features(template_id: &str, json: bool, dir: Option<PathBuf>) -> Result<()> {
    let dir = resolve_dir(dir)?;
    let source = FilesystemTemplateSource::new(&dir);
    let id = TemplateId(template_id.to_string());
    let loaded = source
        .load(&id)
        .map_err(|e| anyhow!("loading `{template_id}`: {e}"))?;

    if json {
        let payload = serde_json::json!(loaded
            .manifest
            .features
            .iter()
            .map(|f| serde_json::json!({
                "id": f.id.0,
                "display_name": f.display_name,
                "default": f.default,
                "requires": f.requires.iter().map(|r| &r.0).collect::<Vec<_>>(),
                "conflicts": f.conflicts.iter().map(|c| &c.0).collect::<Vec<_>>(),
                "stacks": f.stacks,
            }))
            .collect::<Vec<_>>());
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else if loaded.manifest.features.is_empty() {
        println!("Template `{template_id}` declares no features.");
    } else {
        println!(
            "Features in `{}` (v{}):",
            template_id, loaded.manifest.meta.version
        );
        for f in &loaded.manifest.features {
            let mark = if f.default { "✓" } else { " " };
            print!("  {mark} {:<28} {}", f.id.0, f.display_name);
            if !f.requires.is_empty() {
                print!(
                    " (requires {})",
                    f.requires
                        .iter()
                        .map(|r| r.0.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            println!();
        }
    }
    Ok(())
}

fn resolve_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.is_dir() {
            return Err(anyhow!("not a directory: {}", p.display()));
        }
        return Ok(p);
    }
    let cwd = std::env::current_dir().context("getting cwd")?;
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

#[derive(Debug)]
struct TemplateEntry {
    id: String,
    display_name: String,
    version: String,
    stacks: Vec<String>,
    feature_count: usize,
}
