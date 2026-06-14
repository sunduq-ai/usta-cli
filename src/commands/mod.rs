//! Subcommand modules. Each module owns its `Args` struct and a `run` fn.

/// Levenshtein edit distance between two strings. Pure, allocation-light
/// (two rolling rows). Used only for "did you mean?" suggestions, so the
/// inputs are short identifiers.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The closest candidate to `input`, if one is within a sensible edit
/// distance (so `api-fastpai` → `api-fastapi` suggests, but unrelated
/// garbage doesn't). Returns the matched candidate.
pub(crate) fn closest_match<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    candidates
        .into_iter()
        .map(|c| (edit_distance(input, c), c))
        // Allow up to ~⅓ of the candidate length (min 2) — generous enough
        // for a transposition or two, tight enough to avoid nonsense.
        .filter(|(d, c)| *d <= (c.len() / 3).max(2))
        .min_by_key(|(d, c)| (*d, c.len()))
        .map(|(_, c)| c)
}

/// Build a " (did you mean `X`?)" / " (available: a, b, c)" hint for an
/// unknown id, given the valid candidates.
pub(crate) fn suggestion_hint(unknown: &str, candidates: &[String]) -> String {
    let refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
    if let Some(best) = closest_match(unknown, refs.iter().copied()) {
        format!(" (did you mean `{best}`?)")
    } else if candidates.is_empty() {
        String::new()
    } else {
        format!(" (available: {})", refs.join(", "))
    }
}

pub mod add;
pub mod completions;
pub mod doctor;
pub mod extract;
pub mod list;
pub mod new;
pub mod schema;
pub mod update;
pub mod verify;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("api-fastpai", "api-fastapi"), 2); // transposition
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn closest_match_suggests_near_typo() {
        let cands = ["api-fastapi", "web-vite-react", "shared-types"];
        assert_eq!(
            closest_match("api-fastpai", cands.iter().copied()),
            Some("api-fastapi")
        );
        assert_eq!(
            closest_match("nx-monrepo", ["nx-monorepo"].iter().copied()),
            Some("nx-monorepo")
        );
    }

    #[test]
    fn closest_match_rejects_nonsense() {
        let cands = ["api-fastapi", "web-vite-react"];
        assert_eq!(closest_match("zzzzzz", cands.iter().copied()), None);
    }

    #[test]
    fn suggestion_hint_formats() {
        let cands = vec!["api-fastapi".to_string(), "web-vite-react".to_string()];
        assert_eq!(
            suggestion_hint("api-fastpai", &cands),
            " (did you mean `api-fastapi`?)"
        );
        assert!(suggestion_hint("zzzzzz", &cands).starts_with(" (available:"));
        assert_eq!(suggestion_hint("anything", &[]), "");
    }
}
