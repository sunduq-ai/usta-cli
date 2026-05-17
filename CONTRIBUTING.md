# Contributing to `usta-cli`

Thanks for considering a contribution! This project welcomes humans and AI
agents under the same rules.

## Before you start

1. Read [`AGENTS.md`](./AGENTS.md). It is the source of truth for
   architecture invariants, SOLID expectations, testing rules, and
   open-source hygiene.
2. Read [`docs/NON_GOALS.md`](./docs/NON_GOALS.md). Don't propose anything
   on that list without an ADR explaining what changed.
3. For load-bearing decisions, write an ADR in `docs/ADR/`. Use ADR 0001 as
   a template.

## Dev setup

```bash
# pin via rust-toolchain.toml (Rust 1.91+)
cargo build --workspace
cargo test  --workspace
cargo run -p usta -- --help
```

## Local quality checks

These run in CI; run them locally before pushing.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test  --workspace
scripts/check-layers.sh           # crate-graph layering
scripts/check-forbidden-imports.sh # no LLM SDKs, no I/O in core/ports
scripts/check-agent-rules.sh      # superset; runs the above + manifest validation
```

## PR checklist

A PR is mergeable when:

- [ ] Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
      `chore:`, `build:`, `ci:`).
- [ ] `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` pass.
- [ ] Layering rules pass (`scripts/check-layers.sh`).
- [ ] No new dependency without an ADR justifying it.
- [ ] Public items in `usta-core` / `usta-ports` / `usta-app` carry doc
      comments.
- [ ] If the change touches a template, the snapshot e2e test under
      `templates/<id>/tests/` is updated; the diff is acknowledged in the
      PR description.
- [ ] If the change adjusts behavior, `CHANGELOG.md` has an entry under
      `## [Unreleased]`.
- [ ] If this PR proposes anything from `docs/NON_GOALS.md`: linked ADR
      explaining what changed.
- [ ] [If you are an AI agent] you read `AGENTS.md` before this PR.

## Adding a new template

See [`docs/TEMPLATE_AUTHORING.md`](./docs/TEMPLATE_AUTHORING.md). New
templates go under `templates/<id>/` and ship with a snapshot test.

## Adding a new adapter

Add a module under `crates/usta-adapters/src/`, implement the relevant
trait from `usta-ports`, and register it in
`crates/usta-cli/src/wiring.rs`. Don't import the new adapter type
anywhere else.

## Releasing

Releases are cut by a maintainer via `cargo-dist`. CI publishes binaries to
GitHub Releases and (after `crates.io` namespace is secured in P5) the
crate to crates.io.

## Code of Conduct

This project follows the [Contributor Covenant](./CODE_OF_CONDUCT.md).
By participating, you agree to abide by it.
