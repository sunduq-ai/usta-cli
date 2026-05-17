# `usta` — multi-stack project scaffolder

> **usta** (أُسْطَى): "master craftsman" in Arabic. A scaffolding tool that
> stays out of your way and produces real, idiomatic code for any stack.

[![CI](https://github.com/sunduq-ai/usta-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/sunduq-ai/usta-cli/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`usta` is a **single static binary** that scaffolds new projects from
templates, applies features post-hoc, keeps generated projects in sync with
template upgrades, and synthesizes new templates from existing repositories
— **without making any network LLM calls**. The headline use case is
saving tokens on app creation.

## Quick start

```bash
# install (after first release — see "Status" below)
cargo install usta

# discover what's available
usta list templates
usta list features --template nx-monorepo

# preview before scaffolding
usta new my-app --template nx-monorepo --yes --dry-run

# scaffold (non-interactive)
usta new my-app --template nx-monorepo \
    --features api-fastapi,web-vite-react,shared-types --yes

# add a feature later
cd my-app && usta add web-i18n

# detect drift in template-managed files
cd my-app && usta verify

# pull in template improvements (3-way merge against your edits)
cd my-app && usta update

# turn an existing repo into a reusable template (deterministic, no LLM)
usta extract ~/code/my-existing-app --out ./templates --name my-stack

# author tooling
usta schema template > template.schema.json
usta completions zsh > "${fpath[1]}/_usta"
usta doctor
```

## Why `usta` and not …?

| You want… | Use… |
|---|---|
| Just clone a repo without history | `degit` / `giget` |
| Mature Python templates with prompt machinery | `cookiecutter` |
| Same, plus updates to existing projects (Python only) | `copier` |
| One-shot per-stack scaffolders (TS only) | `create-vite`, `create-astro`, `create-t3-app` |
| Multi-stack single binary, deterministic extraction, in-tree updates | **`usta`** |

`copier` is the closest spiritual cousin. `usta` adds: stack-agnostic single
binary, `extract` from any repo, `update`/`add`/`verify` for already-
scaffolded projects.

## What `usta` is not

See [`docs/NON_GOALS.md`](./docs/NON_GOALS.md) for the explicit rejection
list. In short: no Wasm plugins, no hosted registry, no telemetry by
default, no LLM calls in core flows, no lock-in to a single ecosystem.

## How it works

`usta` is a single Rust crate organized as a hexagonal layered engine:

```
src/core ← src/ports ← src/app ← src/adapters ← src/wiring (main.rs)
```

Trait-based ports define what the engine needs (filesystem, prompts,
renderer, package manager, scanner, sanitizer …). Concrete adapters live
behind those traits and are wired only in `src/wiring.rs`. Use cases stay
pure. This is what lets us add a new template, a new package manager, or
a new language sanitizer **without editing the core**.

> Before v0.1.0 the engine lived in five separate crates and the
> dependency rule was Cargo-enforced. For publishing ergonomics
> (`cargo install usta` from a single registry entry) the workspace was
> collapsed into one crate; the architecture survives as `src/` modules.
> See [`docs/ADR/0002-single-crate-collapse.md`](./docs/ADR/0002-single-crate-collapse.md)
> for the rationale.

See [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for the diagram and the
exit-code table, and [`AGENTS.md`](./AGENTS.md) for the rules of the road.

## Status

**v0.1.0 ready for publish** — engine, two built-in templates (`hello-world`,
`nx-monorepo` with 13 features), `extract` / `verify` / `add` / `update`,
schema export, dry-run preview, record/replay, 152 tests passing.

Build from source until the first release lands on crates.io / Homebrew:

```bash
git clone https://github.com/sunduq-ai/usta-cli
cd usta-cli
cargo build --release
./target/release/usta --help
```

Phase-by-phase progress + the v0.2 roadmap live in [`PLAN.md`](./PLAN.md).

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md). Read [`AGENTS.md`](./AGENTS.md)
before opening a PR. We accept changes from humans and AI agents alike,
under the same rules.

Reports of security issues: [`SECURITY.md`](./SECURITY.md).

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([`LICENSE-APACHE`](./LICENSE-APACHE))
- MIT License ([`LICENSE-MIT`](./LICENSE-MIT))

at your option. This matches the standard Rust ecosystem dual-license.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.
