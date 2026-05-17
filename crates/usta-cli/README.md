# `usta` — multi-stack project scaffolder

> **usta** (أُسْطَى): "master craftsman" in Arabic. A scaffolding tool that
> stays out of your way and produces real, idiomatic code for any stack.

`usta` is a **single static binary** that scaffolds new projects from
templates, applies features post-hoc, keeps generated projects in sync
with template upgrades, and synthesizes new templates from existing
repositories — **without making any network LLM calls**.

## Install

```bash
cargo install usta
```

## Quick start

```bash
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

## How it works

`usta` is built as a hexagonal Rust workspace:

```
usta-core ← usta-ports ← usta-app ← usta-adapters ← usta-cli (binary)
```

Trait-based ports define what the engine needs (filesystem, prompts,
renderer, package manager, scanner, sanitizer …). Concrete adapters
live behind those traits and are wired only in the binary.

See the [main repo](https://github.com/sunduq-ai/usta-cli) for the
architecture diagram, the exit-code table, contributing guide, and the
phase-by-phase roadmap.

## License

Dual-licensed under either MIT or Apache-2.0 at your option.
