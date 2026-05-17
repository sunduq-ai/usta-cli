//! Pure JSON-shape deep-merge.
//!
//! Used by the engine to combine multiple feature contributions to the same
//! `package.json` / `pyproject.toml` / `nx.json`, etc. TOML is converted to
//! [`serde_json::Value`] before merging so we have a single set of merge
//! rules (TOML keys map cleanly to JSON; we don't carry TOML-specific
//! datetime values into config-file merges).
//!
//! Semantics:
//!
//! - `Object` ⊕ `Object`: recursive merge; keys in the overlay take
//!   precedence on conflict but recurse into matching sub-objects.
//! - `Array`  ⊕ `Array`:  concatenate, deduplicate by stable equality.
//! - `Scalar` ⊕ anything: overlay wins (last-writer).
//!
//! Determinism: `Object` keys are walked in stable BTreeMap order;
//! deduplication preserves first-seen order (stable across runs).

use serde_json::{Map, Value};

/// Deep-merge `overlay` into `base`, modifying `base` in place.
pub fn deep_merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            // Walk overlay keys in sorted order for determinism.
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            for k in keys {
                let v = &o[k];
                match b.get_mut(k) {
                    Some(existing) => deep_merge(existing, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (Value::Array(b), Value::Array(o)) => {
            for item in o {
                if !b.contains(item) {
                    b.push(item.clone());
                }
            }
        }
        (slot, overlay) => {
            // Scalar / type mismatch: overlay wins.
            *slot = overlay.clone();
        }
    }
}

/// Convenience: merge a list of overlays into a base, in order.
pub fn deep_merge_all(base: &mut Value, overlays: &[Value]) {
    for o in overlays {
        deep_merge(base, o);
    }
}

/// Sort all object keys recursively, in place. Useful before re-serializing
/// so that the on-disk output is deterministic regardless of insertion
/// order.
pub fn canonicalize_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // BTreeMap doesn't preserve insertion order anyway; rebuild as a
            // new ordered Map to guarantee output is sorted.
            let mut sorted: Vec<(String, Value)> =
                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let mut new_map = Map::new();
            for (k, mut v) in sorted {
                canonicalize_keys(&mut v);
                new_map.insert(k, v);
            }
            *map = new_map;
        }
        Value::Array(items) => {
            for item in items {
                canonicalize_keys(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn objects_merge_recursively() {
        let mut a = json!({ "deps": { "left": "1.0" }, "name": "a" });
        let b = json!({ "deps": { "right": "2.0" }, "version": "0.1" });
        deep_merge(&mut a, &b);
        assert_eq!(
            a,
            json!({
                "deps": { "left": "1.0", "right": "2.0" },
                "name": "a",
                "version": "0.1"
            })
        );
    }

    #[test]
    fn arrays_concatenate_and_dedup() {
        let mut a = json!(["x", "y"]);
        let b = json!(["y", "z"]);
        deep_merge(&mut a, &b);
        assert_eq!(a, json!(["x", "y", "z"]));
    }

    #[test]
    fn scalar_overlay_wins() {
        let mut a = json!({ "name": "old" });
        let b = json!({ "name": "new" });
        deep_merge(&mut a, &b);
        assert_eq!(a, json!({ "name": "new" }));
    }

    #[test]
    fn type_mismatch_overlay_wins() {
        let mut a = json!({ "x": 5 });
        let b = json!({ "x": [1, 2] });
        deep_merge(&mut a, &b);
        assert_eq!(a, json!({ "x": [1, 2] }));
    }

    #[test]
    fn deep_merge_all_applies_in_order() {
        let mut a = json!({ "a": 1 });
        deep_merge_all(&mut a, &[json!({ "b": 2 }), json!({ "c": 3 })]);
        assert_eq!(a, json!({ "a": 1, "b": 2, "c": 3 }));
    }

    #[test]
    fn canonicalize_sorts_keys() {
        let mut v = json!({ "z": 1, "a": { "y": 2, "b": 3 } });
        canonicalize_keys(&mut v);
        // Map equality is key-order-insensitive, so test by serializing.
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, r#"{"a":{"b":3,"y":2},"z":1}"#);
    }
}
