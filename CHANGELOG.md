# Changelog

All notable changes to `usta` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
The on-disk template format follows its own SemVer track per template, recorded
in each template's `template.toml` and pinned in every generated project's
`.usta/snapshot.toml`.

## [Unreleased]

### Fixed
- **Anchor markers no longer leak into generated projects.** A scaffold
  that didn't select every optional feature used to leave internal
  `usta:*` marker comments (e.g. `# usta:imports`) in the user's source.
  `new` now strips all residual markers as a finalization pass, so output
  is always marker-free regardless of which features are chosen.
- **`usta completions powershell`** is now accepted (previously only the
  kebab-cased `power-shell` worked).
- **MSRV corrected to 1.85** (was a fictional 1.75; the dependency tree
  requires 1.85). The CI `msrv` job now genuinely enforces it.

### Changed
- **`usta add` re-renders from the template** for the augmented feature
  set and 3-way-merges against the working tree (sharing the `usta update`
  engine), instead of editing live anchor markers. Injection-based
  features apply post-hoc cleanly; if a managed file was edited locally
  the re-render lands in `.usta/proposed/` as a conflict (exit 40). The
  old `AnchorMarkerMissing` failure path is gone.

### Removed
- Unimplemented stub subcommands `search`, `install`, and `self-update`
  (deferred to v0.2; clap now returns a clean "unrecognized subcommand").
- Dead flags that were parsed but never acted on: `new --pm`/`--verify`,
  `extract --interactive`/`--yes`, `update --to`/`--interactive`/`--abort`,
  `add --dry-run`/`--force`.

## [0.1.0] — 2026-05-17

The initial release. The engine + the `nx-monorepo` template land here.
Published to crates.io as a single `usta` crate.

The shipping CLI surface is **9 subcommands**: `new`, `extract`, `update`,
`add`, `verify`, `list`, `doctor`, `completions`, `schema`. The
GitHub-topic template registry (`search` / `install`) and binary
self-update (`self-update`) are deferred to v0.2; their stubs were
removed before release rather than shipped as commands that only error.

### Added

- **Multi-stack scaffolder** (`usta new`) with feature opt-in, JSON/TOML
  deep-merge, and anchor-marker injection (Python `#`, JS/TS/Rust `//`,
  HTML `<!-- -->`, JSX `{/* */}`).
- `usta list templates` and `usta list features --template <id>` for
  discovering installed templates and inspecting features (with `--json`).
- `usta completions <bash|zsh|fish|powershell|elvish>` for shell completions.
- `usta doctor [--json] [--strict]` reporting presence + versions of `git`,
  `node`, `pnpm`, `npm`, `uv`, `python3`, `cargo`, `go`, `docker`.
- `usta schema {template|feature}` emits a Draft-07 JSON Schema for the
  manifest format, suitable for editor autocomplete.
- `usta new --dry-run` previews the scaffold plan (per-file `+`/`~`/`*`
  annotations for write/merge/inject) without touching disk.
- **`usta extract <repo>`**: deterministic repo → template synthesizer
  with `.gitignore` / `.usta-extract-ignore` support, identifier
  substitution, default-noise drop list, feature partitioning. **No LLM
  calls anywhere in the crate graph.**
- **`usta verify`** (exit 41 on drift), **`usta add <feature>`** (post-hoc
  feature application with idempotent merges + smart inject error path),
  **`usta update`** (3-way merge against `.usta/managed.lock`,
  conflicts → `.usta/proposed/<path>`, exit 40).
- **Hexagonal architecture**: originally five crates (`usta-core` /
  `usta-ports` / `usta-app` / `usta-adapters` / binary), collapsed
  before release into a single `usta` crate with the same layout
  preserved as modules (`src/core`, `src/ports`, `src/app`,
  `src/adapters`, `src/commands`, `src/wiring.rs`). The dependency
  rule is now enforced by `scripts/check-agent-rules.sh` and code
  review rather than the Cargo crate graph. See ADR-0002.
- **First crates.io publish**: shipped as a single `usta` crate so
  `cargo install usta` works in one step.
- **Path-traversal write-jail** on the local filesystem adapter, covered
  by a `proptest` property test.
- **`nx-monorepo` template** with 13 features: API (FastAPI / MongoDB /
  JWT auth), Web (Vite + React / router / TanStack Query / i18n), Mobile
  (Expo + NativeWind), shared packages (types / utils / UI), Docker,
  Husky tooling.
- **Renderer filters** for case conversion: `kebab`, `pascal`, `camel`,
  `snake` — case-aware (`HTTPServer` → `http-server`).

### Documentation

- `AGENTS.md` (rules for human and AI contributors), `CLAUDE.md`,
  `.cursor/rules/usta.mdc`.
- `docs/ARCHITECTURE.md`, `docs/NON_GOALS.md`, `docs/EXTRACT.md`,
  `docs/TEMPLATE_AUTHORING.md`, `docs/ADR/0001-hexagonal-architecture.md`,
  `docs/ADR/0002-single-crate-collapse.md`.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, dual
  MIT / Apache-2.0 licenses.

[Unreleased]: https://github.com/sunduq-ai/usta-cli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sunduq-ai/usta-cli/releases/tag/v0.1.0
