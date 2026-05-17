//! Pure plan builder.
//!
//! Given a [`LoadedTemplate`], a list of resolved feature ids, an answer
//! context, and a project root, emit a deterministic [`ScaffoldPlan`].
//!
//! Op order:
//!
//! 1. [`FileOp::Write`] for every base file (in load order).
//! 2. [`FileOp::Write`] for every selected feature's files (in feature-
//!    resolution order, then by path).
//! 3. [`FileOp::Merge`] for every target reached by at least one feature's
//!    merges, with all overlays already deep-merged together.
//! 4. [`FileOp::Inject`] for every target reached by at least one feature's
//!    injections, with all contributions concatenated in feature-resolution
//!    order.

use std::collections::BTreeMap;
use std::path::PathBuf;

use usta_core::loaded::{LoadedTemplate, TemplateContent, TemplateFile};
use usta_core::merge::deep_merge;
use usta_core::plan::{AnchorContribution, FileOp, MergeFormat, ScaffoldPlan};
use usta_core::template::FeatureId;

/// Build a plan from a loaded template + selection + answer context,
/// **including** the base files (for fresh `usta new` runs).
pub fn build_plan(
    template: &LoadedTemplate,
    features: &[FeatureId],
    answers: &BTreeMap<String, serde_json::Value>,
    root: PathBuf,
) -> ScaffoldPlan {
    build_plan_inner(template, features, answers, root, true)
}

/// Build a plan from a loaded template + selection + answer context,
/// **excluding** the base files (for `usta add` runs, where base is
/// already on disk).
pub fn build_features_only_plan(
    template: &LoadedTemplate,
    features: &[FeatureId],
    answers: &BTreeMap<String, serde_json::Value>,
    root: PathBuf,
) -> ScaffoldPlan {
    build_plan_inner(template, features, answers, root, false)
}

fn build_plan_inner(
    template: &LoadedTemplate,
    features: &[FeatureId],
    _answers: &BTreeMap<String, serde_json::Value>,
    root: PathBuf,
    include_base: bool,
) -> ScaffoldPlan {
    let mut ops: Vec<FileOp> = Vec::new();

    if include_base {
        push_files(&mut ops, &template.base_files);
    }

    for fid in features {
        if let Some(files) = template.feature_files.get(fid) {
            push_files(&mut ops, files);
        }
    }

    // Aggregate merges: target → (format, accumulated_value).
    let mut merge_acc: BTreeMap<PathBuf, (MergeFormat, serde_json::Value)> = BTreeMap::new();
    for fid in features {
        if let Some(merges) = template.feature_merges.get(fid) {
            for m in merges {
                merge_acc
                    .entry(m.target.clone())
                    .and_modify(|(_fmt, val)| deep_merge(val, &m.value))
                    .or_insert_with(|| (m.format, m.value.clone()));
            }
        }
    }
    for (target, (format, value)) in merge_acc {
        ops.push(FileOp::Merge {
            path: target,
            format,
            value,
        });
    }

    // Aggregate injections: target → ordered Vec<AnchorContribution>.
    let mut inject_acc: BTreeMap<PathBuf, Vec<AnchorContribution>> = BTreeMap::new();
    for fid in features {
        if let Some(injs) = template.feature_injections.get(fid) {
            for inj in injs {
                let entry = inject_acc.entry(inj.target.clone()).or_default();
                for c in &inj.contributions {
                    entry.push(c.clone());
                }
            }
        }
    }
    for (target, contributions) in inject_acc {
        ops.push(FileOp::Inject {
            path: target,
            contributions,
        });
    }

    ScaffoldPlan { root, ops }
}

fn push_files(ops: &mut Vec<FileOp>, files: &[TemplateFile]) {
    for f in files {
        let bytes_or_render = match &f.content {
            TemplateContent::Verbatim(b) => b.clone(),
            // For Render content, we still emit a Write whose `contents`
            // hold the raw Jinja source. The executor renders it at write
            // time, using the answer context.
            TemplateContent::Render(s) => s.clone().into_bytes(),
        };
        ops.push(FileOp::Write {
            path: f.rel_path.clone(),
            contents: bytes_or_render,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use semver::Version;
    use usta_core::loaded::{LoadedTemplate, TemplateContent, TemplateFile};
    use usta_core::template::{Feature, FeatureId, Template, TemplateId, TemplateMeta};

    use super::*;

    fn loaded(features: Vec<Feature>, base: Vec<TemplateFile>) -> LoadedTemplate {
        let mut feature_files = BTreeMap::new();
        for f in &features {
            feature_files.insert(f.id.clone(), Vec::new());
        }
        LoadedTemplate {
            manifest: Template {
                meta: TemplateMeta {
                    id: TemplateId("t".into()),
                    display_name: "T".into(),
                    version: Version::new(0, 1, 0),
                    min_usta: ">=0.1.0".parse().unwrap(),
                    stacks: vec![],
                },
                features,
                prompts: vec![],
            },
            base_files: base,
            feature_files,
            feature_merges: BTreeMap::new(),
            feature_injections: BTreeMap::new(),
        }
    }

    fn tf(p: &str, body: &str) -> TemplateFile {
        TemplateFile {
            rel_path: PathBuf::from(p),
            content: TemplateContent::Verbatim(body.as_bytes().to_vec()),
        }
    }

    #[test]
    fn base_files_come_first() {
        let t = loaded(vec![], vec![tf("README.md", "hi")]);
        let plan = build_plan(&t, &[], &BTreeMap::new(), PathBuf::from("/out"));
        assert_eq!(plan.ops.len(), 1);
        assert_eq!(plan.root, PathBuf::from("/out"));
    }

    #[test]
    fn feature_order_is_input_order() {
        let mut t = loaded(
            vec![
                Feature {
                    id: FeatureId("a".into()),
                    display_name: "a".into(),
                    default: false,
                    requires: vec![],
                    conflicts: vec![],
                    stacks: vec![],
                },
                Feature {
                    id: FeatureId("b".into()),
                    display_name: "b".into(),
                    default: false,
                    requires: vec![],
                    conflicts: vec![],
                    stacks: vec![],
                },
            ],
            vec![],
        );
        t.feature_files
            .insert(FeatureId("a".into()), vec![tf("a.txt", "A")]);
        t.feature_files
            .insert(FeatureId("b".into()), vec![tf("b.txt", "B")]);

        let plan = build_plan(
            &t,
            &[FeatureId("b".into()), FeatureId("a".into())],
            &BTreeMap::new(),
            PathBuf::from("/out"),
        );
        assert_eq!(plan.ops.len(), 2);
        match (&plan.ops[0], &plan.ops[1]) {
            (FileOp::Write { path: p1, .. }, FileOp::Write { path: p2, .. }) => {
                assert_eq!(p1, &PathBuf::from("b.txt"));
                assert_eq!(p2, &PathBuf::from("a.txt"));
            }
            _ => panic!("expected two Write ops"),
        }
    }
}
