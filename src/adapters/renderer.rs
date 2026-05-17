//! MiniJinja-based template renderer with usta's case-conversion filters.
//!
//! Registered filters (in addition to minijinja's built-ins like `upper`,
//! `lower`, `length`, `default`):
//!
//! | filter | input → output |
//! |--------|----------------|
//! | `kebab` | `MyApp my_app My App` → `my-app` |
//! | `pascal`| `my-app my_app my app` → `MyApp` |
//! | `camel` | `my-app my_app my app` → `myApp` |
//! | `snake` | `MyApp my-app My App` → `my_app` |
//!
//! All four are case-aware: they segment on hyphens, underscores, spaces,
//! and (for inputs without separators) on case transitions
//! (`MyAppHTTP` → `My App HTTP` internally).

use std::collections::BTreeMap;

use crate::ports::renderer::{RenderError, TemplateRenderer};
use minijinja::value::Rest;
use minijinja::{Environment, Value};

/// MiniJinja-backed renderer with usta's filter set.
#[derive(Default)]
pub struct MinijinjaRenderer;

impl MinijinjaRenderer {
    /// Construct.
    pub fn new() -> Self {
        Self
    }

    fn build_env<'a>() -> Environment<'a> {
        let mut env = Environment::new();
        env.add_filter("kebab", filter_kebab);
        env.add_filter("pascal", filter_pascal);
        env.add_filter("camel", filter_camel);
        env.add_filter("snake", filter_snake);
        env
    }
}

impl TemplateRenderer for MinijinjaRenderer {
    fn render(
        &self,
        source: &str,
        context: &BTreeMap<String, serde_json::Value>,
    ) -> Result<String, RenderError> {
        let mut env = Self::build_env();
        env.add_template("t", source)
            .map_err(|e| RenderError::Syntax(e.to_string()))?;
        let tmpl = env
            .get_template("t")
            .map_err(|e| RenderError::Syntax(e.to_string()))?;
        let value = Value::from_serialize(context);
        tmpl.render(&value)
            .map_err(|e| RenderError::Render(e.to_string()))
    }
}

// ─────────────────────────── filter impls ───────────────────────────

fn filter_kebab(input: String, _rest: Rest<Value>) -> String {
    segments(&input).join("-").to_lowercase()
}

fn filter_snake(input: String, _rest: Rest<Value>) -> String {
    segments(&input).join("_").to_lowercase()
}

fn filter_pascal(input: String, _rest: Rest<Value>) -> String {
    segments(&input)
        .into_iter()
        .map(|s| capitalize(&s))
        .collect::<Vec<_>>()
        .join("")
}

fn filter_camel(input: String, _rest: Rest<Value>) -> String {
    let segs = segments(&input);
    let mut out = String::new();
    for (i, s) in segs.iter().enumerate() {
        if i == 0 {
            out.push_str(&s.to_lowercase());
        } else {
            out.push_str(&capitalize(s));
        }
    }
    out
}

/// Split `s` into lowercase word segments, normalizing hyphens, underscores,
/// spaces, and case transitions.
///
/// Examples:
///
/// - `"my-app"` → `["my", "app"]`
/// - `"my_app"` → `["my", "app"]`
/// - `"My App"` → `["my", "app"]`
/// - `"myApp"` → `["my", "app"]` (lower→Upper transition)
/// - `"HTTPServer"` → `["http", "server"]` (abbreviation boundary)
/// - `"MyAppHTTP"` → `["my", "app", "http"]` (Upper→Upper then trailing abbrev)
fn segments(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();

    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' || c == '_' || c.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }

        if c.is_ascii_uppercase() && !cur.is_empty() {
            let prev_lower = cur
                .chars()
                .last()
                .map(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
                .unwrap_or(false);
            // 1. Plain case transition: "myApp" → "my" + "App"
            if prev_lower {
                out.push(std::mem::take(&mut cur));
            } else if cur.chars().all(|p| p.is_ascii_uppercase()) {
                // 2. Abbreviation boundary: when we're inside a run of
                //    uppercase ("HTTP") and the next character (after `c`)
                //    is lowercase, `c` starts a new word ("Server"):
                //    we split BEFORE pushing `c`.
                let next_is_lower = chars
                    .peek()
                    .map(|n| n.is_ascii_lowercase())
                    .unwrap_or(false);
                if next_is_lower && !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
        }

        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter().map(|s| s.to_lowercase()).collect()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(template: &str, ctx_pairs: &[(&str, &str)]) -> String {
        let r = MinijinjaRenderer::new();
        let mut ctx = BTreeMap::new();
        for (k, v) in ctx_pairs {
            ctx.insert(k.to_string(), serde_json::json!(v));
        }
        r.render(template, &ctx).unwrap()
    }

    #[test]
    fn renders_simple_var() {
        assert_eq!(
            render("hello {{ name }}", &[("name", "usta")]),
            "hello usta"
        );
    }

    #[test]
    fn kebab_filter_lowercases_and_hyphenates() {
        assert_eq!(render("{{ x | kebab }}", &[("x", "MyApp")]), "my-app");
        assert_eq!(render("{{ x | kebab }}", &[("x", "my_app")]), "my-app");
        assert_eq!(render("{{ x | kebab }}", &[("x", "My App")]), "my-app");
    }

    #[test]
    fn snake_filter_lowercases_and_underscores() {
        assert_eq!(render("{{ x | snake }}", &[("x", "MyApp")]), "my_app");
        assert_eq!(render("{{ x | snake }}", &[("x", "my-app")]), "my_app");
    }

    #[test]
    fn pascal_filter_capitalizes_segments() {
        assert_eq!(render("{{ x | pascal }}", &[("x", "my-app")]), "MyApp");
        assert_eq!(render("{{ x | pascal }}", &[("x", "my_app")]), "MyApp");
        assert_eq!(
            render("{{ x | pascal }}", &[("x", "round-trip")]),
            "RoundTrip"
        );
    }

    #[test]
    fn camel_filter_lowercases_first_segment() {
        assert_eq!(render("{{ x | camel }}", &[("x", "my-app")]), "myApp");
        assert_eq!(render("{{ x | camel }}", &[("x", "my_app")]), "myApp");
        assert_eq!(render("{{ x | camel }}", &[("x", "MyApp")]), "myApp");
    }

    #[test]
    fn segments_handles_abbreviation_boundary() {
        assert_eq!(
            render("{{ x | kebab }}", &[("x", "HTTPServer")]),
            "http-server"
        );
    }

    #[test]
    fn upper_lower_built_in_filters_still_work() {
        assert_eq!(render("{{ x | upper }}", &[("x", "abc")]), "ABC");
        assert_eq!(render("{{ x | lower }}", &[("x", "ABC")]), "abc");
    }
}
