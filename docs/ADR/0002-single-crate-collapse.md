# ADR 0002: Collapse the workspace into a single crate

- **Status**: Accepted
- **Date**: 2026-05-17
- **Supersedes**: [ADR-0001](./0001-hexagonal-architecture.md)

## Context

We want `cargo install usta` to Just Work for first-time users, and we
want `cargo publish` to be a single command in our release pipeline. The
five-crate workspace (`usta-core`, `usta-ports`, `usta-app`,
`usta-adapters`, `usta-cli`) blocks both:

- crates.io requires each crate to be published separately, in
  dependency order, with each version bump propagated through every
  downstream `Cargo.toml`. For a binary whose internal crate graph is
  an implementation detail, this is pure ceremony.
- A user typing `cargo install usta-cli` (or whatever name we settled
  on) has to discover which of the five names is the binary. The
  workspace structure leaks into our distribution UX.

The hexagonal layout from ADR-0001 has otherwise paid for itself —
use cases stay testable with fakes, adapters are swappable, the no-LLM
guarantee is auditable. We want to keep that layout; we just don't
want it to drive our publish story.

## Decision

Collapse the workspace into a single `usta` crate. Preserve the
hexagonal layout as a module hierarchy under `src/`:

```
src/core/      (was usta-core)
src/ports/     (was usta-ports)
src/app/       (was usta-app)
src/adapters/  (was usta-adapters)
src/commands/  (was usta-cli/src/commands)
src/wiring.rs  (was usta-cli/src/wiring.rs)
```

The dependency rule from ADR-0001 still holds — `core` and `ports`
import nothing else, `app` imports only `core` + `ports`, `adapters`
implement port traits, and only `wiring.rs` names concrete adapter
types. The compiler no longer enforces it; reviewers and
`scripts/check-agent-rules.sh` do.

## Consequences

**Pros**

- `cargo install usta` works with one published crate.
- One version number, one `cargo publish`, one tag — release pipeline
  shrinks from "publish 5 crates in order" to "publish one crate".
- Faster incremental compiles (no inter-crate boundaries to round-trip
  through; cargo can parallelize more aggressively at the module
  level).
- Simpler mental model for new contributors — one `Cargo.toml`, one
  `target/`, one set of dependencies.
- Per-crate README files are gone; `README.md` at the repo root is the
  single source of truth.

**Cons**

- Layer discipline is no longer compiler-enforced. Nothing stops
  `app::scaffold` from importing `adapters::fs::LocalFs` except
  reviewer attention and the agent-rules script. ADR-0001 listed this
  as the reason _not_ to collapse; we are accepting the trade because
  the codebase has stabilized and the layers have proven self-enforcing
  in practice over the project's initial development.
- If we ever want to expose the engine as a Rust library (so other
  binaries can embed `usta` programmatically), we will need to re-split
  — at minimum carving `core` + `ports` + `app` back into their own
  crate. The module boundaries make this mechanical when the time
  comes.

## Alternatives considered

- **Keep the workspace, publish only `usta-cli` to crates.io with the
  others as path dependencies.** Rejected: crates.io rejects path-only
  dependencies; every transitive crate must be published. There is no
  "publish only the binary" mode.
- **Keep the workspace, rename `usta-cli` → `usta` to fix the install
  UX.** Helps the install command but leaves the five-publish release
  ceremony in place.
- **Split into two crates: `usta-engine` (library) + `usta` (binary).**
  Captures the "future Rust-library consumers" case from day one. Not
  worth two publishes today when zero such consumers exist; the
  re-split is mechanical if and when one shows up.
