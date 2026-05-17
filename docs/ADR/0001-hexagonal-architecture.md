# ADR 0001: Hexagonal architecture across five crates

- **Status**: Accepted
- **Date**: 2026-05-08
- **Phase**: P0

## Context

`usta` will scaffold many tech stacks (TS, Python, Go, Rust, …), apply
features post-hoc, keep generated projects in sync with template upgrades,
and synthesize templates from existing repos via deterministic extraction.
Each of these is a use case; each can grow new requirements over time.

We need an architecture where:

- New templates / package managers / sanitizers / detectors land **without
  editing core logic**.
- The deterministic-extraction guarantee is mechanically enforceable
  (no accidental network calls, no hidden LLM dependencies).
- Use cases are unit-testable without a real filesystem / TTY / git.
- A future contributor (human or AI) cannot easily violate the layering
  even by accident.

## Decision

Five Cargo crates, dependency-rule-as-crate-graph:

```
usta-core   ←  usta-ports  ←  usta-app  ←  usta-adapters  ←  usta-cli (binary)
```

- `usta-core` and `usta-ports` are I/O-free.
- `usta-app` is generic over ports; depends on `usta-core` + `usta-ports`
  only.
- Concrete adapters live in `usta-adapters` and are wired only inside the
  binary's `wiring.rs`.

The crate graph itself enforces most of the rule (cyclic deps fail to
compile). `scripts/check-layers.sh` covers the rest (forbidden-import
greps, no `usta-adapters` import from `usta-app`).

## Consequences

**Pros**

- The compiler refuses many common abstraction leaks.
- Use cases are unit-testable with `InMemoryFs` and equivalent fakes.
- New templates and adapters are additive — no edits to `core`/`app`.
- The "no LLM in core flows" guarantee can be enforced by inspecting the
  `core`/`ports`/`app` `Cargo.toml`s.

**Cons**

- Five crates is more ceremony than one. Mitigated by clear directory
  layout and a single `Cargo.toml` workspace file.
- Generic constraints in `usta-app` add some line noise versus
  `Box<dyn Trait>`. Worth it for unit-test ergonomics and zero-cost
  monomorphization at the binary boundary.

## Alternatives considered

- **Single crate, modules instead of crates.** Rejected: layer-rule violations
  become invisible. The compiler would happily let `app::scaffold` import
  `adapters::fs::LocalFs`.
- **Three crates (`core`, `app`, `bin`).** Rejected: leaks I/O concerns into
  `app` and makes the no-LLM guarantee harder to enforce mechanically.
- **Plugin / Wasm-based adapters.** Rejected; see `docs/NON_GOALS.md`.
