# CLAUDE.md

**Read [`AGENTS.md`](./AGENTS.md) first.** It is the source of truth for
contributors (human and AI). This file only adds Claude-Code-specific notes.

## Quick orientation

- Cargo workspace, five crates: `usta-core`, `usta-ports`, `usta-app`,
  `usta-adapters`, `usta-cli` (binary).
- The binary entrypoint: `crates/usta-cli/src/main.rs`.
- The composition root: `crates/usta-cli/src/wiring.rs` — the only place
  concrete adapters meet trait-bound use cases.

## Commands you'll need often

```bash
cargo check --workspace          # fast compile check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo run -p usta -- --help
scripts/check-layers.sh          # layer-rule check (matches CI)
scripts/check-agent-rules.sh     # superset of the above
```

## Sub-agent hints

- For long-running searches across the codebase, prefer the `Explore` agent
  with a "very thorough" breadth.
- For independent feature folders under a template (during P2), spawn one
  general-purpose subagent per feature in parallel — they don't share files.

## What NOT to do

- Don't add a network LLM dependency. Anywhere. See `AGENTS.md` §3.
- Don't import `usta-adapters` types from `usta-app`. The compiler will
  refuse. Don't try to work around it.
- Don't edit `docs/NON_GOALS.md` to remove an item without an ADR.
- Don't run `git push --force` or `git reset --hard` without explicit
  confirmation.

## Live status

Phase progress lives in [`PLAN.md`](./PLAN.md). Update it as you complete
work — that file is the user's window into where things stand.
