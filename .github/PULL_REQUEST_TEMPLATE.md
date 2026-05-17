<!--
Thanks for your contribution!

Read AGENTS.md before opening this PR if you haven't already.
-->

## What

<!-- One paragraph: what does this change do, why now? -->

## How

<!-- Brief: notable design choices, alternatives considered, perf/safety impact. -->

## Checklist

- [ ] Conventional Commits in the title (`feat:`, `fix:`, `docs:`, `refactor:`,
      `test:`, `chore:`, `build:`, `ci:`).
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] `scripts/check-layers.sh` passes.
- [ ] No new crate dependency, OR an ADR justifying it is included.
- [ ] Public items in `usta-core` / `usta-ports` / `usta-app` carry doc
      comments.
- [ ] If a template was changed, the snapshot diff is acknowledged in the
      description.
- [ ] If behavior changed, `CHANGELOG.md` has an entry under `## [Unreleased]`.
- [ ] **This PR does not propose anything from
      [`docs/NON_GOALS.md`](../docs/NON_GOALS.md). If it does, a linked ADR
      explains what changed.**
- [ ] [If you are an AI agent] you read `AGENTS.md` before opening this PR.

## Linked issue

<!-- Closes #N -->
