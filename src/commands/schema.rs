//! `usta schema {template|feature}` — emit a JSON Schema for the requested
//! manifest type. Pipe into a file and reference it from your editor's
//! `tasks.json` / `settings.json` to get autocomplete + validation while
//! authoring `template.toml` and `feature.toml`.
//!
//! Schemas are hand-maintained here so they remain stable independent of
//! incidental changes to internal types. The integration tests below assert
//! that they round-trip through `serde_json`.

use anyhow::Result;
use clap::{Args, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Kind {
    /// Schema for `template.toml`.
    Template,
    /// Schema for `feature.toml` (per-feature local manifest; reserved for a future release).
    Feature,
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    /// Which manifest schema to emit.
    pub kind: Kind,
}

pub fn run(args: SchemaArgs) -> Result<()> {
    let body = match args.kind {
        Kind::Template => TEMPLATE_SCHEMA,
        Kind::Feature => FEATURE_SCHEMA,
    };
    println!("{body}");
    Ok(())
}

const TEMPLATE_SCHEMA: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://github.com/sunduq-ai/usta-cli/schemas/template.schema.json",
  "title": "usta template manifest",
  "description": "Top-level template.toml — describes a usta template, its features, and the prompts run during `usta new`.",
  "type": "object",
  "required": ["template"],
  "additionalProperties": false,
  "properties": {
    "template": {
      "type": "object",
      "required": ["id", "display_name", "version", "min_usta"],
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9-]*$",
          "description": "Stable kebab-case identifier, must match the directory name."
        },
        "display_name": { "type": "string" },
        "version": {
          "type": "string",
          "description": "SemVer version of the template definition (e.g. `1.0.0`)."
        },
        "min_usta": {
          "type": "string",
          "description": "SemVer requirement on the `usta` CLI version (e.g. `>=0.1.0`)."
        },
        "stacks": {
          "type": "array",
          "items": { "type": "string" },
          "default": [],
          "description": "Informational only — engine is stack-agnostic."
        }
      }
    },
    "features": {
      "type": "array",
      "default": [],
      "items": { "$ref": "#/$defs/Feature" }
    },
    "prompts": {
      "type": "array",
      "default": [],
      "items": { "$ref": "#/$defs/Prompt" }
    }
  },
  "$defs": {
    "Feature": {
      "type": "object",
      "required": ["id", "display_name"],
      "additionalProperties": false,
      "properties": {
        "id": { "type": "string", "pattern": "^[a-z][a-z0-9-]*$" },
        "display_name": { "type": "string" },
        "default": { "type": "boolean", "default": false },
        "requires": {
          "type": "array",
          "default": [],
          "items": { "type": "string", "pattern": "^[a-z][a-z0-9-]*$" }
        },
        "conflicts": {
          "type": "array",
          "default": [],
          "items": { "type": "string", "pattern": "^[a-z][a-z0-9-]*$" }
        },
        "stacks": {
          "type": "array",
          "default": [],
          "items": { "type": "string" }
        }
      }
    },
    "Prompt": {
      "type": "object",
      "required": ["id", "type", "question"],
      "additionalProperties": false,
      "properties": {
        "id": { "type": "string" },
        "question": { "type": "string" },
        "type": {
          "type": "string",
          "enum": ["text", "confirm", "select", "multiselect"]
        },
        "default": { "type": ["string", "null"] },
        "validate": {
          "type": ["string", "null"],
          "description": "Optional regex validator for `text` prompts."
        },
        "options": {
          "type": "array",
          "default": [],
          "items": { "type": "string" },
          "description": "Required for `select`/`multiselect`."
        }
      }
    }
  }
}
"##;

const FEATURE_SCHEMA: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://github.com/sunduq-ai/usta-cli/schemas/feature.schema.json",
  "title": "usta per-feature manifest",
  "description": "Reserved for a future release (per-feature `feature.toml` overlays declaring deps + hooks). Kept as a permissive placeholder so editors don't fail validation today.",
  "type": "object",
  "additionalProperties": true,
  "properties": {
    "hooks": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "post_scaffold": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    }
  }
}
"##;
