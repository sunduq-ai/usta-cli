# Changelog

All notable changes to `usta` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
The on-disk template format follows its own SemVer track per template, recorded
in each template's `template.toml` and pinned in every generated project's
`.usta/snapshot.toml`.

## [Unreleased]

_Nothing yet._

## [0.4.0] — 2026-05-17

### Added
- **`usta new` now closes the scaffold loop.** After writing files it
  initializes a git repository with an initial commit and installs
  dependencies, so a scaffolded project is runnable immediately:
  - **git** — `git init -b main` + stage + an initial commit. Uses your
    configured identity, falling back to a throwaway one only if none is
    set. Skipped (with a note) if `git` isn't on PATH, and it never
    re-initializes or commits over a directory that's already a repo.
  - **dependencies** — detects the project's package managers and runs
    their install in each relevant directory: `pnpm install` for a pnpm
    workspace (or `npm install` for a plain `package.json`), `uv sync`
    per `pyproject.toml`, `go mod download` per `go.mod`. A pnpm workspace
    install covers all member packages (no duplicate npm runs). The detection
    walk skips `node_modules`/`.venv`/`target`/`.git`/`.usta`.
  - Install runs **before** the initial commit, so generated lockfiles
    (`pnpm-lock.yaml`, `uv.lock`, …) land in that commit and the working
    tree is clean afterward. Dependency directories stay gitignored.

### Changed
- **Behavior change:** `usta new` previously only wrote files. By default it
  now also runs git init + dependency install. The pre-existing `--no-git`
  and `--no-install` flags — formerly inert no-ops — now genuinely gate these
  steps. Pass both to restore the old "just write files" behavior.
- Every post-scaffold action is best-effort: a missing tool is skipped with a
  note and a failing tool warns, but neither makes `usta new` exit non-zero —
  the files are already on disk.

### Fixed
- Package-manager availability detection no longer misreports `go` as absent
  (`go` rejects `--version`; the check now treats a successful spawn, not a
  zero exit, as "installed").

## [0.3.1] — 2026-05-17

Docs only — no code changes.

### Changed
- **README:** added a step-by-step Installation section, including PATH
  setup (verify with `usta --version`; per-shell instructions to add
  `~/.cargo/bin` on bash/zsh/fish, plus Windows guidance) and optional
  shell-completions setup. Published so the crates.io page carries the
  same guide as GitHub.

## [0.3.0] — 2026-05-17

Makes `cargo install usta` actually usable out of the box, plus friendlier
errors. Found by dogfooding the published binary as a fresh user.

### Added
- **Built-in templates are embedded in the binary.** `cargo install usta`
  followed by `usta new my-app --template nx-monorepo` now works with no
  `--templates-dir` — previously it dead-ended on "no templates directory
  found" because the templates shipped in the crate tarball weren't
  installed alongside the binary. The built-ins (`hello-world`,
  `nx-monorepo`) are embedded via `include_dir` and extracted to a
  per-version cache dir (`$XDG_CACHE_HOME/usta/` etc.) on first use.
  Resolution order is unchanged for everyone else: `--templates-dir` /
  `USTA_TEMPLATES_DIR` → a `templates/` dir found by walking up from the
  cwd → the embedded built-ins.
- **"Did you mean?" suggestions** for a mistyped `--template` or feature
  id (e.g. `--features api-fastpai` → "did you mean `api-fastapi`?"),
  with the full list of valid ids when nothing is close.

### Fixed
- **hello-world generated a broken project.** Its `base/index.js` carried
  `{{ project_name }}` but wasn't a `.j2`, so it shipped the literal
  template variable; and the `with-router` feature injected
  `require('./router')` without shipping `router.js`, so `node index.js`
  crashed. Both fixed; regression tests now run the output, not just
  inspect it.
- **Doubled phrasing** in the invalid-project-name error
  (`invalid project name \`X\`: invalid project name: …`) collapsed to a
  single clear message.

### Internal
- The four duplicated `resolve_templates_dir` / `resolve_dir` copies
  (`new`, `add`, `update`, `list`) are now one shared
  `wiring::resolve_templates_dir`.

## [0.2.1] — 2026-05-17

Packaging and docs only — no code or behavior changes.

### Changed
- **Leaner published crate.** Dev-only files (`.cursor/`, `.github/`,
  `scripts/`, `.editorconfig`, `clippy.toml`, `rustfmt.toml`,
  `rust-toolchain.toml`) are now excluded from the `cargo publish`
  tarball (200 → 182 files). Everything the README links to plus
  `templates/`, `src/`, and `tests/` still ships.
- **README** now states the `Rust 1.85+` requirement up front in the
  Quick start, so users on an older toolchain know before `cargo install`.

## [0.2.0] — 2026-05-17

Breaking release. Trims the CLI to only what works, fixes a marker-leak
bug in generated projects, and corrects the MSRV. `cargo install usta`
continues to work unchanged.

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

This release shipped 12 subcommands, three of which (`search`, `install`,
`self-update`) were non-functional stubs that exited 64. They were
removed in 0.2.0; the working surface is the other 9.

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
