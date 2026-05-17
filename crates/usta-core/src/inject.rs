//! Anchor-marker injection.
//!
//! Templates can leave marker comments in their files for features to
//! contribute into:
//!
//! ```python
//! # usta:imports
//! ```
//!
//! Each feature's `injections/<target>.inject.toml` lists `(marker,
//! content)` pairs; the engine accumulates them in feature-resolution order
//! and replaces every marker line with the joined contributions, stripping
//! the marker line itself from the output.
//!
//! Supported marker prefixes (so the same mechanism works across
//! languages):
//!
//! - `#`  — Python, shell, TOML, YAML
//! - `//` — JS/TS/Rust/Go/C-family
//! - `<!--`...`-->` — HTML/XML/Markdown
//! - `{/*`...`*/}` — JSX/TSX (for use inside JSX trees)
//!
//! A marker line is "any line whose trimmed content begins with one of the
//! prefixes above and contains the literal `usta:<id>`". We stay
//! deliberately simple: no nesting, no parameters.

// `AnchorContribution` is defined once in `plan.rs` and re-exported here so
// users of either module see the same type.
pub use crate::plan::AnchorContribution;

/// Apply `contributions` to `source`, replacing every marker line with the
/// joined contributions for that marker (no marker line in output).
///
/// If a marker has no contribution, its anchor line is removed entirely
/// (otherwise the generated file would carry stale comments).
pub fn apply_injections(source: &str, contributions: &[AnchorContribution]) -> String {
    // Group contributions by marker, preserving order.
    use std::collections::BTreeMap;
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in contributions {
        grouped
            .entry(c.marker.clone())
            .or_default()
            .push(c.content.clone());
    }

    let mut out = String::with_capacity(source.len());
    let mut iter = source.lines().peekable();
    let preserves_trailing_newline = source.ends_with('\n');

    while let Some(line) = iter.next() {
        if let Some(marker) = detect_marker(line) {
            // Skip the marker line. Insert grouped contributions, joined
            // by the line's leading indent so the inserted text stays
            // aligned with the surrounding code.
            if let Some(parts) = grouped.get(&marker) {
                let indent: String = line
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                for (i, p) in parts.iter().enumerate() {
                    for (j, sub) in p.lines().enumerate() {
                        if i > 0 || j > 0 {
                            out.push('\n');
                        }
                        if !sub.is_empty() {
                            out.push_str(&indent);
                            out.push_str(sub);
                        }
                    }
                }
                // Always end inserted block with a newline if the next line
                // exists.
                if iter.peek().is_some() {
                    out.push('\n');
                }
            }
            continue;
        }
        out.push_str(line);
        if iter.peek().is_some() {
            out.push('\n');
        }
    }
    if preserves_trailing_newline && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// If `line` is a marker comment, return the marker name (e.g. `"usta:imports"`).
fn detect_marker(line: &str) -> Option<String> {
    let trimmed = line.trim_start();

    // Order matters: longer/more-specific prefixes first so `{/*` and `<!--`
    // win over `#`/`//`.
    let body = if let Some(rest) = trimmed.strip_prefix("{/*") {
        rest.trim_start().trim_end_matches("*/}").trim()
    } else if let Some(rest) = trimmed.strip_prefix("<!--") {
        rest.trim_start().trim_end_matches("-->").trim()
    } else if let Some(rest) = trimmed.strip_prefix("//") {
        rest.trim_start()
    } else if let Some(rest) = trimmed.strip_prefix("#") {
        rest.trim_start()
    } else {
        return None;
    };

    // The marker must be the entire (rest of) the line, after stripping
    // common comment closers.
    let body = body.trim();
    if body.starts_with("usta:") && !body.contains(char::is_whitespace) {
        Some(body.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(marker: &str, content: &str) -> AnchorContribution {
        AnchorContribution {
            marker: marker.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn injects_into_python_anchor() {
        let src = "import os\n# usta:imports\nprint('hi')\n";
        let out = apply_injections(
            src,
            &[
                c("usta:imports", "from foo import bar"),
                c("usta:imports", "from baz import qux"),
            ],
        );
        assert_eq!(
            out,
            "import os\nfrom foo import bar\nfrom baz import qux\nprint('hi')\n"
        );
    }

    #[test]
    fn injects_into_javascript_anchor() {
        let src = "import a from './a';\n// usta:imports\nfunction main() {}\n";
        let out = apply_injections(src, &[c("usta:imports", "import b from './b';")]);
        assert_eq!(
            out,
            "import a from './a';\nimport b from './b';\nfunction main() {}\n"
        );
    }

    #[test]
    fn injects_into_html_anchor() {
        let src = "<head>\n<!-- usta:meta -->\n</head>\n";
        let out = apply_injections(src, &[c("usta:meta", r#"<meta charset="utf-8">"#)]);
        assert_eq!(out, "<head>\n<meta charset=\"utf-8\">\n</head>\n");
    }

    #[test]
    fn unused_anchor_is_stripped() {
        let src = "before\n# usta:nope\nafter\n";
        let out = apply_injections(src, &[]);
        assert_eq!(out, "before\nafter\n");
    }

    #[test]
    fn preserves_indent() {
        let src = "fn main() {\n    // usta:body\n}\n";
        let out = apply_injections(src, &[c("usta:body", "println!(\"hi\");\nlet x = 1;")]);
        assert_eq!(
            out,
            "fn main() {\n    println!(\"hi\");\n    let x = 1;\n}\n"
        );
    }

    #[test]
    fn ignores_non_marker_comments() {
        let src = "# this is a normal comment\n# usta:foo\n";
        let out = apply_injections(src, &[c("usta:foo", "added")]);
        assert_eq!(out, "# this is a normal comment\nadded\n");
    }

    #[test]
    fn injects_into_jsx_anchor() {
        let src = "<App>\n  {/* usta:children */}\n</App>\n";
        let out = apply_injections(src, &[c("usta:children", "<Hello />")]);
        assert_eq!(out, "<App>\n  <Hello />\n</App>\n");
    }
}
