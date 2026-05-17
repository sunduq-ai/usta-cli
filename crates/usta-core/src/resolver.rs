//! Pure feature resolution.
//!
//! Given a [`Template`] and a set of selected feature ids, decide:
//!
//! 1. whether the selection is consistent (no missing requires, no
//!    conflicts);
//! 2. a stable, deterministic order in which to apply features
//!    (topological by `requires`, breaking ties alphabetically by id).
//!
//! This module performs **no I/O**. It is the kind of code we want to
//! unit-test exhaustively.

use std::collections::{BTreeMap, BTreeSet};

use crate::errors::DomainError;
use crate::template::{Feature, FeatureId, Template};

/// Resolve a selection against a template.
///
/// Returns the resolved order of feature ids (topological + alphabetical),
/// or a [`DomainError`] explaining the violation.
pub fn resolve(
    template: &Template,
    selected: &BTreeSet<FeatureId>,
) -> Result<Vec<FeatureId>, DomainError> {
    let by_id: BTreeMap<&FeatureId, &Feature> =
        template.features.iter().map(|f| (&f.id, f)).collect();

    // 1. Every selected id is known to the template.
    for id in selected {
        if !by_id.contains_key(id) {
            return Err(DomainError::UnknownFeature(id.0.clone()));
        }
    }

    // 2. All `requires` are present in the selection (transitively).
    //    We auto-include required features rather than erroring out; the
    //    UI surfaces the auto-additions to the user upstream.
    let mut effective: BTreeSet<FeatureId> = selected.clone();
    let mut frontier: Vec<FeatureId> = selected.iter().cloned().collect();
    while let Some(id) = frontier.pop() {
        let feat = by_id.get(&id).expect("checked above");
        for req in &feat.requires {
            if !by_id.contains_key(req) {
                return Err(DomainError::MissingRequiredFeature {
                    required: req.0.clone(),
                    by: id.0.clone(),
                });
            }
            if effective.insert(req.clone()) {
                frontier.push(req.clone());
            }
        }
    }

    // 3. No two selected features conflict.
    for id in &effective {
        let feat = by_id[id];
        for other in &feat.conflicts {
            if effective.contains(other) {
                let (a, b) = if id.0 < other.0 {
                    (id.0.clone(), other.0.clone())
                } else {
                    (other.0.clone(), id.0.clone())
                };
                return Err(DomainError::FeatureConflict { a, b });
            }
        }
    }

    // 4. Topological sort by `requires`. Kahn's algorithm with alphabetical
    //    tie-breaking for determinism.
    let mut in_degree: BTreeMap<FeatureId, usize> =
        effective.iter().map(|id| (id.clone(), 0)).collect();
    for id in &effective {
        for req in &by_id[id].requires {
            if effective.contains(req) {
                *in_degree.get_mut(id).expect("present") += 1;
            }
        }
    }

    let mut ready: BTreeSet<FeatureId> = in_degree
        .iter()
        .filter_map(|(id, n)| if *n == 0 { Some(id.clone()) } else { None })
        .collect();

    let mut out: Vec<FeatureId> = Vec::with_capacity(effective.len());
    while let Some(id) = ready.iter().next().cloned() {
        ready.remove(&id);
        out.push(id.clone());
        // Reduce in-degree of features that required `id`.
        for other in &effective {
            if by_id[other].requires.contains(&id) {
                let n = in_degree.get_mut(other).expect("present");
                *n -= 1;
                if *n == 0 {
                    ready.insert(other.clone());
                }
            }
        }
    }

    if out.len() != effective.len() {
        // A cycle was detected. Manifest validation should reject these
        // earlier; treat as an invalid manifest if it ever reaches here.
        return Err(DomainError::InvalidManifest(format!(
            "cyclic feature dependency in template `{}`",
            template.id().0
        )));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::*;
    use crate::template::{Prompt, TemplateId, TemplateMeta};

    fn fid(s: &str) -> FeatureId {
        FeatureId(s.to_string())
    }

    fn feat(id: &str, requires: &[&str], conflicts: &[&str]) -> Feature {
        Feature {
            id: fid(id),
            display_name: id.to_string(),
            default: false,
            requires: requires.iter().map(|s| fid(s)).collect(),
            conflicts: conflicts.iter().map(|s| fid(s)).collect(),
            stacks: vec![],
        }
    }

    fn template(features: Vec<Feature>) -> Template {
        Template {
            meta: TemplateMeta {
                id: TemplateId("test".into()),
                display_name: "test".into(),
                version: Version::new(1, 0, 0),
                min_usta: ">=0.1.0".parse().unwrap(),
                stacks: vec![],
            },
            features,
            prompts: vec![] as Vec<Prompt>,
        }
    }

    fn selected(ids: &[&str]) -> BTreeSet<FeatureId> {
        ids.iter().map(|s| fid(s)).collect()
    }

    #[test]
    fn empty_selection_resolves_to_empty() {
        let t = template(vec![feat("a", &[], &[])]);
        let r = resolve(&t, &selected(&[])).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn unknown_feature_errors() {
        let t = template(vec![feat("a", &[], &[])]);
        let err = resolve(&t, &selected(&["b"])).unwrap_err();
        assert!(matches!(err, DomainError::UnknownFeature(_)));
    }

    #[test]
    fn topological_order_is_stable() {
        let t = template(vec![
            feat("c", &["b"], &[]),
            feat("b", &["a"], &[]),
            feat("a", &[], &[]),
        ]);
        let r = resolve(&t, &selected(&["a", "b", "c"])).unwrap();
        assert_eq!(
            r.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn auto_includes_required_features() {
        let t = template(vec![
            feat("c", &["b"], &[]),
            feat("b", &["a"], &[]),
            feat("a", &[], &[]),
        ]);
        let r = resolve(&t, &selected(&["c"])).unwrap();
        assert_eq!(
            r.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn missing_requires_target_errors() {
        let t = template(vec![feat("a", &["nope"], &[])]);
        let err = resolve(&t, &selected(&["a"])).unwrap_err();
        assert!(matches!(err, DomainError::MissingRequiredFeature { .. }));
    }

    #[test]
    fn conflict_errors() {
        let t = template(vec![feat("a", &[], &["b"]), feat("b", &[], &[])]);
        let err = resolve(&t, &selected(&["a", "b"])).unwrap_err();
        assert!(matches!(err, DomainError::FeatureConflict { .. }));
    }

    #[test]
    fn cycle_is_an_invalid_manifest() {
        let t = template(vec![feat("a", &["b"], &[]), feat("b", &["a"], &[])]);
        let err = resolve(&t, &selected(&["a", "b"])).unwrap_err();
        assert!(matches!(err, DomainError::InvalidManifest(_)));
    }

    #[test]
    fn alphabetical_tiebreak_is_deterministic() {
        // Two independent features — order must be alphabetical.
        let t = template(vec![feat("z", &[], &[]), feat("a", &[], &[])]);
        let r = resolve(&t, &selected(&["z", "a"])).unwrap();
        assert_eq!(
            r.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
            vec!["a", "z"]
        );
    }
}
