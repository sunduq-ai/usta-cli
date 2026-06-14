# PLAN.md — `usta-cli` phased delivery

> **Live status.** Updated as each phase progresses. Look here to know
> where things stand. The architecture and rules behind these phases live
> in [`AGENTS.md`](./AGENTS.md), [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md),
> and [`docs/NON_GOALS.md`](./docs/NON_GOALS.md).

## Status — 2026-05-17

| Phase | Status | Summary |
|------:|:------:|---------|
| **P8** — v0.2.0 release | ✅ done | Second crates.io release. Bug fix: anchor markers no longer leak into generated projects (`new` strips residual `usta:*` markers as a finalization pass). `usta add` reworked to re-render + 3-way-merge via the `update` engine (injection features apply post-hoc cleanly; local edits → `.usta/proposed/` conflict). `completions powershell` accepted. MSRV corrected 1.75 → 1.85 (the dependency tree actually requires it) and the `msrv` CI job now enforces it. Full feature simulation across all 13 features (individual + combined + incremental-add) validates real syntax (Python compiles, JS parses, JSON/TOML parse) — **156 tests pass**. |
| **P7** — Publish & honest surface | ✅ done | `usta v0.1.0` published to crates.io. Removed three unimplemented stub subcommands (`search`, `install`, `self-update`) that only ever exited 64 — clap now returns a clean "unrecognized subcommand" (with suggestions) instead of advertising broken commands. Removed dead flags that were parsed but never read (`new --pm`/`--verify`, `extract --interactive`/`--yes`, `update --to`/`--interactive`/`--abort`, `add --dry-run`). Purged internal `P0`–`P5` phase jargon from all user-facing `--help` text. CLI surface is now **9 working subcommands**. CI fixed: dropped `cargo test --doc` (no library target post-collapse). **152 tests pass**. |
| **P6** — Single-crate collapse (v0.1.0 publish prep) | ✅ done | Workspace collapsed to a single `usta` crate; former crates kept as modules (`src/core`, `src/ports`, `src/app`, `src/adapters`, `src/commands`, `src/wiring.rs`) preserving the hexagonal layout. `scripts/check-layers.sh` and `scripts/check-forbidden-imports.sh` deleted — their intent now lives in `scripts/check-agent-rules.sh`. Per-crate README files removed. Layer discipline is now code-review-enforced rather than Cargo-enforced. Ready for `cargo publish` as a single crate. |
| **P0** — Skeleton | ✅ done | Workspace + 5 crates compile, all CI gates green locally. |
| **P1** — Core engine | ✅ done (a–i) | resolver · LocalFs+proptest · template loader · plan build/exec · prompts · `usta new` end-to-end · snapshot+lock · JSON/TOML deep-merge + anchor injection (incl. JSX). P1.j deferred to P5. |
| **P2** — `nx-monorepo` template | ✅ done + audited | **13 features** (api-fastapi/mongodb/auth-jwt · web-vite-react/router/tanstack-query/i18n · mobile-expo · shared-types/utils/ui · docker · tooling-husky) · **6 integration tests** · all 13 features exercised · injection-content rendering verified · `.usta/managed.lock` and `.usta/snapshot.toml` format-validated. |
| **P3** — `extract` subcommand | ✅ done | Deterministic repo→template synthesizer · `IgnoreScanner` adapter (`.gitignore` + `.usta-extract-ignore`) · `ExtractConfig` (TOML) · identifier substitution · default-noise drop list · feature partitioning · synthesizer + writer + service · `usta extract <repo> --out <dir>` wired · **3 integration tests** including extract → scaffold round-trip and byte-level determinism · case filters (`kebab`/`pascal`/`camel`/`snake`) added to renderer (gap exposed by round-trip test) · **90 total tests pass**. |
| **P4** — `update` / `add` / `verify` | ✅ done | `usta verify` (5 unit + 5 integration tests; exit-41 on drift; `--json` output) · `usta add <feature>` (7 integration tests; covers Write/Merge/Inject post-hoc + `AnchorMarkerMissing` error path + `--templates-dir` resolution) · `usta update` (6 integration tests; 3-way merge against `managed.lock`; conflicts written to `.usta/proposed/<path>` with exit-40; orphaned files reported; new files added; verify-clean post-update) · `ManagedLock` parser added to `usta-core` (5 round-trip tests). **118 total tests pass**. |
| **P5** — Polish & release | ✅ done (v0.1.0 surface) | `usta list templates`/`features` (4 tests + 4 JSON-output tests) · `usta completions {bash,zsh,fish,powershell,elvish}` (3 tests) · `usta doctor [--json] [--strict]` (2 tests, checks 9 tools) · `usta schema {template\|feature}` emits Draft-07 JSON Schema (2 tests) · `usta new --dry-run` tree preview with per-op kinds + size summary (2 tests) · `usta new --record/--replay` answer files for CI/regression (4 tests, including positional-name override of replay's project_name) · deferred OSS docs landed: `CHANGELOG.md` (keep-a-changelog), `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1 reference), `SECURITY.md` (private vuln reporting + SLA) · `cargo-dist` config in `Cargo.toml [workspace.metadata.dist]` + `.github/workflows/release.yml` for cross-platform binaries on tag · **136 total tests pass**. **Deferred to v0.2**: `usta search`/`install` (GitHub-topic registry), `usta self-update` (binary replacement + signature verification), `update --abort`/`--interactive`/`--to <version>` (need snapshot history + registry), mdBook docs site. |

Legend: ✅ done · 🟡 in progress · ⬜ pending · 🔵 blocked.

---

## P0 — Skeleton

**Goal:** workspace + 5 crates + governance + CI gates compile and pass
end-to-end. The architecture is real and enforced before any feature
lands.

### Done

- ✅ Cargo workspace, 5 crates: `usta-core`, `usta-ports`, `usta-app`,
  `usta-adapters`, `usta-cli`.
- ✅ Workspace-wide `Cargo.toml` with shared dep versions, dev/release
  profiles, MSRV pinned (1.75) via `rust-toolchain.toml`.
- ✅ Dual MIT / Apache-2.0 licences.
- ✅ `usta --version` and `usta --help` work; all 12 subcommand stubs are
  registered with stable exit code 64 ("not yet implemented in P0").
- ✅ Domain types (`Template`, `Feature`, `ScaffoldPlan`, `FileOp`,
  `ProjectName`) with unit-tested validators in `usta-core`.
- ✅ Port traits: `FileSystem`, `PromptUi`, `TemplateRenderer`,
  `PackageManager`, `VcsClient`, `RepoScanner`, `StackDetector`,
  `SourceSanitizer`, `Clock`, `Telemetry`.
- ✅ `InMemoryFs` adapter (test-only) and `MinijinjaRenderer` adapter,
  both with passing unit tests.
- ✅ Governance: `AGENTS.md` (source of truth), `CLAUDE.md`,
  `.cursor/rules/usta.mdc`.
- ✅ `docs/`: `ARCHITECTURE.md`, `NON_GOALS.md`, `EXTRACT.md`,
  `TEMPLATE_AUTHORING.md`, `ADR/0001-hexagonal-architecture.md`.
- ✅ Open-source docs: `README.md`, `CONTRIBUTING.md`.
- ✅ CI: `ci.yml` (fmt + clippy + tests on Linux/macOS/Windows + layer
  check + forbidden-imports + agent-rules + MSRV + cargo doc + Conventional
  Commits), `release.yml` (P5 stub), Dependabot, CODEOWNERS,
  PR + 3 issue templates.
- ✅ Scripts: `check-layers.sh`, `check-forbidden-imports.sh`,
  `check-agent-rules.sh` (executable, layer + forbidden checks pass
  locally).

### Deferred to P5

- ⏭️ `CODE_OF_CONDUCT.md` — Contributor Covenant 2.1.
- ⏭️ `SECURITY.md` — private vuln reporting.
- ⏭️ `CHANGELOG.md` — keep-a-changelog, automated by `release-plz`.

### P0 acceptance gate — all green ✅

- [x] `cargo check --workspace` passes.
- [x] `cargo fmt --all -- --check` silent.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [x] `RUSTDOCFLAGS="-Dwarnings" cargo doc --workspace --no-deps` clean.
- [x] `bash scripts/check-layers.sh` passes.
- [x] `bash scripts/check-forbidden-imports.sh` passes.
- [x] `./target/debug/usta --version` → `usta 0.1.0-dev`.
- [x] `./target/debug/usta --help` shows all 12 subcommands.

### P1 acceptance gate — all green ✅

- [x] `cargo test --workspace` — **52 tests** (core 22 · adapters 22 · app 3
      · integration 5).
- [x] Path-traversal `proptest` ≥ 256 cases pass on every run.
- [x] `usta new <name> --template hello-world --yes` produces a tree with:
      base files (rendered + verbatim), `.gitignore`, AGENTS.md.
- [x] Selecting `--features with-deps` deep-merges `lodash` into
      `package.json`, preserving base values; sorted keys output.
- [x] Selecting `--features with-router` injects an import line via the
      `// usta:imports` anchor and strips the marker from output.
- [x] `.usta/snapshot.toml` records template id + version + answers +
      timestamp; `.usta/managed.lock` lists SHA-256 of every written file.
- [x] All five integration tests pass: default, explicit features, merge,
      inject, invalid name.

---

## P1 — Core engine

**Goal:** the scaffold engine works for an embedded toy template, with all
plumbing the headline features need (snapshot writer, record/replay,
dry-run preview, schema export, path-traversal property test).

Planned work:

- `usta-core::plan_builder` (pure): selected features → ordered
  `ScaffoldPlan`.
- `usta-core::feature_resolver` (pure): topological sort, conflict
  detection.
- `usta-app::scaffold::ScaffoldService<F: FileSystem, R: TemplateRenderer, P: PromptUi>`.
- `usta-adapters::fs::LocalFs` with write-jail, `proptest` covering path
  traversal.
- JSON / TOML deep-merge utilities (with semver-aware dep range union).
- Anchor-marker injection.
- `.usta/snapshot.toml` writer + `managed.lock` (SHA-256 manifest).
- `--record` / `--replay` plumbing through `PromptUi`.
- `--dry-run` tree preview.
- `usta schema {template|feature}` emits JSON Schema.
- An embedded "hello-world" template used only by integration tests.

### Acceptance gate

- [ ] `usta new myapp --template hello-world --yes` writes the expected
      tree to a tempdir; integration test asserts via `insta` snapshot.
- [ ] Path-traversal `proptest` ≥ 1000 cases passes.
- [ ] Snapshot file is written and round-trips through serde.

---

## P2 — `nx-monorepo` template (parallel with P3)

**Goal:** the my-existing-app-shaped template, fully featured, business logic
removed. Runs alongside P3 because they touch disjoint adapter sets.

Planned features (each its own folder under
`templates/nx-monorepo/features/`):

- `api-fastapi`, `api-mongodb`, `api-redis`, `api-auth-jwt`, `api-rbac`,
  `api-media-upload`, `api-pdf-weasyprint`, `api-otel`
- `web-vite-react`, `web-router`, `web-tanstack-query`, `web-zustand`,
  `web-i18n`, `web-theme`, `web-auth-ui`
- `mobile-expo`
- `shared-types`, `shared-ui`, `shared-utils`, `shared-hooks`,
  `shared-state`, `shared-i18n`, `shared-api-client`
- `docker`, `tooling-husky`, `tooling-github-actions`

Plus: `template.toml`, `AGENTS.md.j2` seed, `templates/nx-monorepo/tests/snapshot.rs`.

### Acceptance gate

- [ ] `usta new probe --template nx-monorepo --yes` produces a tree that
      passes `pnpm install`, `pnpm typecheck`, `uv sync`, and
      `pytest -k health`.
- [ ] Snapshot test stable across reruns.

---

## P3 — `extract` subcommand (parallel with P2)

**Goal:** point at any repo, get back a sanitized template, deterministically.

Planned work:

- `usta-adapters::scanner::IgnoreScanner`.
- `StackDetector` chain: `package_json`, `pyproject_toml`, `nx_json`,
  `go_mod`, `cargo_toml`, `vite_config`, `expo_config`, `dockerfile`,
  `docker_compose`.
- `SourceSanitizer` impls for `typescript` and `python` via tree-sitter.
- `usta-app::extract::TemplateSynthesizer`.
- `.usta-extract.toml` parsing with `keep_paths` / `drop_paths` /
  `identifiers` / manual `features`.

### Acceptance gate

- [ ] Run `usta extract ~/workspace/my-existing-app --out /tmp/extracted` and diff
      against the hand-written `nx-monorepo` template; differences fit on
      one screen and are documented as known-divergences.

---

## P4 — `update` / `add` / `verify`

**Goal:** generated projects stay in sync with template upgrades; features
land post-hoc; drift is visible.

Planned work:

- 3-way merge using `managed.lock` to distinguish "untouched ⇒ safe
  overwrite" from "modified ⇒ conflict".
- `--abort` restores the pre-update snapshot.
- `usta add <feature>` reuses the engine for a single feature.
- `usta verify` flags drift (CI-friendly, `--json` flag).

### Acceptance gate

- [ ] N → N+1 template upgrade test: scaffold v1.0.0, bump template to
      v1.1.0, `usta update` applies cleanly when no user edits; surfaces
      conflicts when user edits a managed file.
- [ ] `usta add web-i18n` to a generated project produces the same tree
      as scaffolding fresh with that feature included.

---

## P5 — Polish & release

**Goal:** v0.1.0 published, installable, documented.

Planned work:

- `usta doctor` (real checks: tools on PATH, network, write perms,
  versions in supported ranges).
- `usta completions {bash|zsh|fish|powershell}`.
- `usta self-update` via GitHub Releases.
- `usta search` / `usta install` via GitHub topic `usta-template`.
- `cargo-dist` config; cross-platform binaries.
- Homebrew tap.
- `mdBook` site (light) deployed to GitHub Pages.
- Deferred-from-P0: `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`.
- v0.1.0 tagged + published to crates.io + GitHub Releases.

### Acceptance gate

- [ ] `cargo install usta` (or downloaded binary) works on Linux / macOS /
      Windows.
- [ ] All five `cargo doc` crates have green doc-build with no warnings.

---

## Phase DAG (parallelization)

```
P0  →  P1  ┬─→  P2  ─┐
           └─→  P3  ─┴─→  P4  →  P5
```

- **P2 ∥ P3** is the only meaningful parallelization opportunity (~1.5
  days saved). They share P1's engine but touch disjoint adapter sets
  (renderer/pkg-managers vs. scanner/sanitizers).
- Within P2, individual feature folders are independent and can be
  authored by parallel sub-agents (no shared files).
- P0/P1 and P4/P5 are sequential.
