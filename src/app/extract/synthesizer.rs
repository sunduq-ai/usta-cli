//! Pure synthesizer.
//!
//! Takes scanned files + an [`ExtractConfig`] and produces an
//! [`ExtractedTemplate`] ready for serialization.
//!
//! Determinism: same scanned input + same config → identical
//! `ExtractedTemplate` byte-for-byte.

use std::path::{Path, PathBuf};

use crate::core::extract::{
    apply_identifier_substitutions, default_drop_globs, looks_like_text, ExtractConfig,
};
use crate::core::paths::to_forward_slashes;
use crate::core::template::{Feature, FeatureId, Template, TemplateId, TemplateMeta};
use globset::{Glob, GlobSet, GlobSetBuilder};
use semver::{Version, VersionReq};

use super::ExtractError;

/// Output of synthesis — a [`Template`] manifest plus a flat list of files
/// already laid out under their template-relative paths
/// (`base/...`, `features/<id>/files/...`).
#[derive(Debug, Clone)]
pub struct ExtractedTemplate {
    /// Synthesized template manifest.
    pub manifest: Template,
    /// Files to write under `templates/<template_id>/`.
    pub files: Vec<TemplateOutFile>,
    /// Number of source-repo files dropped by the noise filter.
    pub dropped: usize,
}

/// One on-disk file in the synthesized template tree.
#[derive(Debug, Clone)]
pub struct TemplateOutFile {
    /// Path relative to `templates/<template_id>/`
    /// (e.g. `base/package.json.j2`).
    pub rel_path: PathBuf,
    /// File body, post-substitution.
    pub bytes: Vec<u8>,
    /// Whether this file was rendered as text (i.e. went through identifier
    /// substitution and may be a `.j2`).
    pub is_text: bool,
}

/// Synthesize a template from scanned files.
///
/// The `scanned` slice is `(rel_path, bytes)` pairs as produced by reading
/// each file the scanner returned.
pub fn synthesize(
    scanned: &[(PathBuf, Vec<u8>)],
    config: &ExtractConfig,
) -> Result<ExtractedTemplate, ExtractError> {
    // Compile glob sets up front so we can match per-file in the hot loop.
    let drop_set = build_glob_set(default_drop_globs())?;
    let user_drop_set = build_glob_set_strs(&config.drop_paths)?;
    let user_keep_set = build_glob_set_strs(&config.keep_paths)?;

    let feature_sets: Vec<(FeatureId, GlobSet, &str)> = config
        .features
        .iter()
        .map(|f| -> Result<_, ExtractError> {
            let set = build_glob_set_strs(&f.paths)?;
            let display = f.display_name.as_deref().unwrap_or(&f.id);
            Ok((FeatureId(f.id.clone()), set, display))
        })
        .collect::<Result<_, _>>()?;

    let mut files: Vec<TemplateOutFile> = Vec::new();
    let mut dropped = 0usize;

    for (rel_path, bytes) in scanned {
        // Decide drop vs. keep.
        let is_default_dropped = drop_set.is_match(rel_path);
        let is_user_dropped = user_drop_set.is_match(rel_path);
        let is_user_kept = user_keep_set.is_match(rel_path);

        if (is_default_dropped && !is_user_kept) || is_user_dropped {
            dropped += 1;
            continue;
        }

        // Apply identifier substitutions to text files.
        let (out_bytes, is_text, content_changed) = if looks_like_text(bytes) {
            let original = std::str::from_utf8(bytes).unwrap_or("");
            let substituted = apply_identifier_substitutions(original, &config.identifiers);
            let changed = substituted != original;
            (substituted.into_bytes(), true, changed)
        } else {
            (bytes.clone(), false, false)
        };

        // Decide bucket: feature partition (first match wins) or base/.
        let bucket = feature_sets
            .iter()
            .find(|(_, set, _)| set.is_match(rel_path))
            .map(|(id, _, _)| Bucket::Feature(id.clone()))
            .unwrap_or(Bucket::Base);

        // Compose the destination path inside the template tree. Add a
        // `.j2` suffix only when substitution actually changed the file
        // (to avoid forcing every file through the renderer at scaffold).
        //
        // `PathBuf::join` uses the host's separator (`\` on Windows), but
        // `rel_path` was already normalized to `/` by the scanner — joining
        // would produce mixed `base\src/lib.rs`. Re-normalize so every
        // path written into the template tree (and ultimately into
        // `.usta/managed.lock` for projects scaffolded from it) is portable.
        let composed = match bucket {
            Bucket::Base => PathBuf::from("base").join(rel_path),
            Bucket::Feature(ref fid) => PathBuf::from("features")
                .join(&fid.0)
                .join("files")
                .join(rel_path),
        };
        let mut tree_rel = to_forward_slashes(&composed);
        if content_changed {
            tree_rel = with_appended_extension(&tree_rel, "j2");
        }

        files.push(TemplateOutFile {
            rel_path: tree_rel,
            bytes: out_bytes,
            is_text,
        });
    }

    // Stable order for deterministic output.
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let manifest = build_manifest(config);

    Ok(ExtractedTemplate {
        manifest,
        files,
        dropped,
    })
}

#[derive(Debug, Clone)]
enum Bucket {
    Base,
    Feature(FeatureId),
}

fn build_manifest(config: &ExtractConfig) -> Template {
    let id = config
        .template_id
        .clone()
        .unwrap_or_else(|| "extracted".to_string());
    let display_name = config
        .template_display_name
        .clone()
        .unwrap_or_else(|| id.clone());

    let features: Vec<Feature> = config
        .features
        .iter()
        .map(|f| Feature {
            id: FeatureId(f.id.clone()),
            display_name: f.display_name.clone().unwrap_or_else(|| f.id.clone()),
            default: f.default,
            requires: vec![],
            conflicts: vec![],
            stacks: vec![],
        })
        .collect();

    Template {
        meta: TemplateMeta {
            id: TemplateId(id),
            display_name,
            version: Version::new(0, 1, 0),
            // Conservative — generated templates are valid for the version
            // of `usta` that produced them. Authors can tighten manually.
            min_usta: VersionReq::parse(">=0.1.0").expect("static reqstring"),
            stacks: config.stacks.clone(),
        },
        features,
        prompts: vec![],
    }
}

fn build_glob_set(patterns: &[&str]) -> Result<GlobSet, ExtractError> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        builder.add(
            Glob::new(p).map_err(|e| ExtractError::InvalidConfig(format!("glob `{p}`: {e}")))?,
        );
    }
    builder
        .build()
        .map_err(|e| ExtractError::InvalidConfig(format!("globset: {e}")))
}

fn build_glob_set_strs(patterns: &[String]) -> Result<GlobSet, ExtractError> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        builder.add(
            Glob::new(p).map_err(|e| ExtractError::InvalidConfig(format!("glob `{p}`: {e}")))?,
        );
    }
    builder
        .build()
        .map_err(|e| ExtractError::InvalidConfig(format!("globset: {e}")))
}

fn with_appended_extension(p: &Path, ext: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn pair(p: &str, c: &str) -> (PathBuf, Vec<u8>) {
        (PathBuf::from(p), c.as_bytes().to_vec())
    }

    #[test]
    fn drops_default_noise() {
        let scanned = vec![
            pair("README.md", "hi"),
            pair("node_modules/lodash/index.js", "junk"),
            pair("apps/web/dist/bundle.js", "junk"),
            pair("Cargo.lock", "junk"),
            pair(".DS_Store", "junk"),
        ];
        let cfg = ExtractConfig::default();
        let out = synthesize(&scanned, &cfg).unwrap();
        let paths: Vec<String> = out
            .files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert_eq!(paths, vec!["base/README.md".to_string()]);
        assert_eq!(out.dropped, 4);
    }

    #[test]
    fn user_keep_overrides_default_drop() {
        let scanned = vec![pair("Cargo.lock", "lock-content")];
        let mut cfg = ExtractConfig::default();
        cfg.keep_paths.push("Cargo.lock".into());
        let out = synthesize(&scanned, &cfg).unwrap();
        let paths: Vec<String> = out
            .files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert_eq!(paths, vec!["base/Cargo.lock".to_string()]);
    }

    #[test]
    fn user_drop_takes_precedence_over_default_keep() {
        let scanned = vec![
            pair("README.md", "hi"),
            pair("apps/web/secret-config.json", "shhh"),
        ];
        let mut cfg = ExtractConfig::default();
        cfg.drop_paths.push("**/secret-config.json".into());
        let out = synthesize(&scanned, &cfg).unwrap();
        let paths: Vec<String> = out
            .files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert_eq!(paths, vec!["base/README.md".to_string()]);
    }

    #[test]
    fn substituted_files_get_j2_suffix() {
        let scanned = vec![pair("package.json", r#"{"name": "my-existing-app"}"#)];
        let mut cfg = ExtractConfig::default();
        cfg.identifiers
            .insert("my-existing-app".into(), "{{ project_name }}".into());
        let out = synthesize(&scanned, &cfg).unwrap();
        assert_eq!(out.files.len(), 1);
        assert_eq!(
            out.files[0].rel_path.display().to_string(),
            "base/package.json.j2"
        );
        assert_eq!(out.files[0].bytes, br#"{"name": "{{ project_name }}"}"#);
    }

    #[test]
    fn unsubstituted_files_keep_original_extension() {
        let scanned = vec![pair("README.md", "no template vars here")];
        let cfg = ExtractConfig::default();
        let out = synthesize(&scanned, &cfg).unwrap();
        assert_eq!(out.files.len(), 1);
        assert_eq!(
            out.files[0].rel_path.display().to_string(),
            "base/README.md"
        );
    }

    #[test]
    fn binary_files_pass_through_verbatim() {
        let scanned = vec![(PathBuf::from("static/data.bin"), vec![0u8, 1, 2, 0, 3, 4])];
        let mut cfg = ExtractConfig::default();
        cfg.identifiers.insert("anything".into(), "X".into());
        let out = synthesize(&scanned, &cfg).unwrap();
        assert_eq!(out.files.len(), 1);
        assert!(!out.files[0].is_text);
        assert_eq!(out.files[0].bytes, vec![0u8, 1, 2, 0, 3, 4]);
        // No `.j2` suffix because content didn't change.
        assert_eq!(
            out.files[0].rel_path.display().to_string(),
            "base/static/data.bin"
        );
    }

    #[test]
    fn feature_partition_routes_files() {
        use crate::core::extract::FeaturePartition;
        let scanned = vec![
            pair("apps/api/main.py", "from x import y"),
            pair("apps/web/main.tsx", "import App from './App';"),
            pair("README.md", "hi"),
        ];
        let cfg = ExtractConfig {
            features: vec![
                FeaturePartition {
                    id: "api".into(),
                    display_name: Some("API".into()),
                    paths: vec!["apps/api/**".into()],
                    default: true,
                },
                FeaturePartition {
                    id: "web".into(),
                    display_name: Some("Web".into()),
                    paths: vec!["apps/web/**".into()],
                    default: true,
                },
            ],
            ..Default::default()
        };
        let out = synthesize(&scanned, &cfg).unwrap();
        let paths: Vec<String> = out
            .files
            .iter()
            .map(|f| f.rel_path.display().to_string())
            .collect();
        assert_eq!(
            paths,
            vec![
                "base/README.md".to_string(),
                "features/api/files/apps/api/main.py".to_string(),
                "features/web/files/apps/web/main.tsx".to_string(),
            ]
        );
        assert_eq!(out.manifest.features.len(), 2);
    }

    #[test]
    fn manifest_uses_template_id_from_config() {
        let scanned = vec![pair("a", "b")];
        let cfg = ExtractConfig {
            template_id: Some("my-stack".into()),
            template_display_name: Some("My Stack".into()),
            stacks: vec!["typescript".into(), "python".into()],
            ..Default::default()
        };
        let out = synthesize(&scanned, &cfg).unwrap();
        assert_eq!(out.manifest.meta.id.0, "my-stack");
        assert_eq!(out.manifest.display_name(), "My Stack");
        assert_eq!(out.manifest.meta.stacks, vec!["typescript", "python"]);
    }

    #[test]
    fn synthesis_is_deterministic_across_input_order() {
        let a = vec![pair("a.txt", "a"), pair("b.txt", "b"), pair("c.txt", "c")];
        let b = vec![pair("c.txt", "c"), pair("a.txt", "a"), pair("b.txt", "b")];
        let cfg = ExtractConfig::default();
        let r1 = synthesize(&a, &cfg).unwrap();
        let r2 = synthesize(&b, &cfg).unwrap();
        let p1: Vec<_> = r1.files.iter().map(|f| f.rel_path.clone()).collect();
        let p2: Vec<_> = r2.files.iter().map(|f| f.rel_path.clone()).collect();
        assert_eq!(p1, p2);
    }

    #[test]
    fn invalid_glob_surfaces_error() {
        let scanned = vec![pair("a", "b")];
        let mut cfg = ExtractConfig::default();
        cfg.drop_paths.push("[invalid".into());
        let err = synthesize(&scanned, &cfg).unwrap_err();
        assert!(matches!(err, ExtractError::InvalidConfig(_)));
    }

    #[test]
    fn empty_substitutions_doesnt_force_j2() {
        let scanned = vec![pair("README.md", "hello my-existing-app")];
        let cfg = ExtractConfig {
            identifiers: BTreeMap::new(),
            ..Default::default()
        };
        let out = synthesize(&scanned, &cfg).unwrap();
        assert_eq!(
            out.files[0].rel_path.display().to_string(),
            "base/README.md"
        );
    }
}
