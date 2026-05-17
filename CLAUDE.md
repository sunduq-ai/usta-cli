# CLAUDE.md

**Read [`AGENTS.md`](./AGENTS.md) first.** It is the source of truth for
contributors (human and AI). This file only adds Claude-Code-specific notes.

## Quick orientation

- Single crate (`usta`) at the repo root. The crate ships both the binary
  and the engine code; the hexagonal architecture survives as `src/`
  modules.
- The binary entrypoint: `src/main.rs`.
- The composition root: `src/wiring.rs` — the only place concrete adapters
  from `crate::adapters` meet trait-bound use cases from `crate::app`.
- Templates live at `templates/` (read at runtime via `--templates-dir`).
- See `docs/ADR/0002-single-crate-collapse.md` for why this isn't a
  workspace anymore.

## Commands you'll need often

```bash
cargo check                       # fast compile check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run -- --help
scripts/check-agent-rules.sh      # fmt + clippy + tests + hygiene
```

## Sub-agent hints

- For long-running searches across the codebase, prefer the `Explore`
  agent with a "very thorough" breadth.
- For independent feature folders under a template, spawn one
  general-purpose subagent per feature in parallel — they don't share
  files.

## What NOT to do

- Don't add a network LLM dependency. Anywhere. See `AGENTS.md` §3.
- Don't import from `crate::adapters` inside `crate::app`. The compiler
  used to refuse this (when the layers were separate crates); now it's a
  code-review responsibility but the rule still stands.
- Don't edit `docs/NON_GOALS.md` to remove an item without an ADR.
- Don't run `git push --force` or `git reset --hard` without explicit
  confirmation.

## Live status

Phase progress lives in [`PLAN.md`](./PLAN.md). Update it as you complete
work — that file is the user's window into where things stand.
