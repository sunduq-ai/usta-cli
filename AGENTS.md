# AGENTS.md — Rules for humans and AI agents working on `usta-cli`

> **Source of truth.** `CLAUDE.md` and `.cursor/rules/usta.mdc` point here.
> Read this file before opening a PR or making non-trivial changes.

## 0. Mission

`usta` is a **multi-stack, deterministic, single-binary** project scaffolder.
It scaffolds new projects from templates, applies features post-hoc, keeps
generated projects in sync with template upgrades, and synthesizes templates
from existing repositories — **without making any network LLM calls**. Saving
tokens on app creation is the headline benefit.

## 1. Architecture invariants (code-review enforced)

The module dependency rule is the law:

```
crate::core   ←  crate::ports
                    ↑
                crate::app
                    ↑
                crate::adapters
                    ↑
                crate::wiring (composition root, called from main.rs)
```

Until v0.1.0 these were five separate crates and the dependency rule was
**compiler-enforced** via Cargo's crate graph. For publishing ergonomics
(`cargo install usta` from a single registry entry) the workspace was
collapsed into one crate; the hexagonal layout survives as `src/` modules.
The trade-off: layer discipline is now a **code-review responsibility**.
See `docs/ADR/0002-single-crate-collapse.md` for the rationale.

- `crate::core` and `crate::ports` MUST NOT use any I/O. Forbidden: `tokio`,
  `reqwest`, `git2`, `std::process`, `std::fs` writes/reads. They may use
  `std::path::Path` as a value type.
- `crate::app` MAY depend on `crate::core` and `crate::ports`. It MUST NOT
  import from `crate::adapters`.
- Concrete adapter types (anything from `crate::adapters`) are mentioned
  ONLY inside `src/wiring.rs`. No other module should import an adapter
  struct.
- `Box<dyn Trait>` only at the composition root. Use generic type parameters
  with trait bounds in `crate::app` use cases.

`scripts/check-agent-rules.sh` runs in CI and includes the soft hygiene
checks. If layer creep becomes a recurring problem, add a grep-based lint.

## 2. SOLID checklist

Every PR must be answerable "yes" to each:

- **SRP**: each module/struct has one reason to change. Traits with > 5
  methods get split.
- **OCP**: new behavior arrives behind a new trait, not by editing an
  existing concrete type's `match` arms.
- **LSP**: trait impls obey the trait's documented invariants
  (e.g. `FileSystem::write` MUST refuse to escape the write jail).
- **ISP**: ports stay narrow. If two callers need disjoint subsets of a
  trait, split the trait.
- **DIP**: use cases depend on `crate::ports` traits, never on
  `crate::adapters` types. Code review enforces this; the compiler used to
  via the crate graph before v0.1.0's single-crate collapse.

## 3. The `extract` invariants

- Default operation is fully deterministic. Same input → same output.
- No network LLM calls anywhere in the engine.
- Optional local-only AI (e.g. Ollama on `127.0.0.1`) MAY be added in a
  future minor version, behind a port + an opt-in flag, behind a feature
  flag, never as the default. `crate::core`/`crate::ports`/`crate::app`
  MUST NOT import a network HTTP client.
- Sanitized output never contains source-repo identifiers (verified by
  snapshot tests).

## 4. The `update` invariants

- Never silently overwrite a user-modified file. If a file's hash differs
  from `managed.lock`, surface a conflict the way `git merge` does.
- `--abort` always restorable from the previous `.usta/snapshot`.
- Template version bumps follow SemVer. Major bumps require a documented
  migration in `CHANGELOG.md`.

## 5. Adding a template

- A new template lives at `templates/<id>/`.
- It MUST NOT require edits to `crate::core` or `crate::app`. If you find
  yourself editing them, the abstraction is wrong — open an ADR before
  shipping.
- It MUST ship: `template.toml`, an `AGENTS.md.j2` seed for the generated
  project, an e2e test under `templates/<id>/tests/`, and pass `usta verify`
  immediately after scaffold.
- Renaming a feature is a breaking template change → bump template
  `version`'s major.

## 6. Testing

- Every use case in `crate::app` has a unit test using in-memory adapters
  defined locally in `#[cfg(test)]` modules (since the layers no longer
  live in separate crates, in-memory adapters are scoped per-test-module).
- Every adapter has at least one integration test against the real backend
  (filesystem via `tempfile`, child process via real binaries on PATH).
- Every template has a snapshot e2e test that scaffolds into `tempfile::tempdir()`
  and asserts the file tree (`insta::assert_yaml_snapshot!`).
- A `proptest` property test asserts no `FileOp` ever resolves outside the
  configured write jail.
- No `#[cfg(test)]` flags inside production logic — the only acceptable use
  is to expose extra constructors for tests.

## 7. Errors

- Engine modules (`core`, `ports`, `app`, `adapters`) use **typed** errors
  via `thiserror`. `anyhow` only at the binary boundary (`main.rs`,
  `commands/`, `wiring.rs`).
- Every variant maps to a stable exit code documented in
  `docs/ARCHITECTURE.md`.
- User-facing error messages name the file path or template id involved.

## 8. Open-source hygiene

- **Conventional Commits** required (`feat:`, `fix:`, `docs:`, `refactor:`,
  `test:`, `chore:`, `build:`, `ci:`). CI rejects PRs that don't conform.
- Every public item in `core`, `ports`, `app` carries a doc comment.
  `cargo doc --no-deps` runs in CI.
- New dependencies require an ADR justifying them and the licence (only
  MIT / Apache-2.0 / BSD / MPL-2.0 / ISC accepted).
- MSRV bumps require an ADR.
- `cargo public-api` runs in CI; API drift requires either a changelog
  entry or a major version bump.

## 9. Safety

- The CLI MUST NEVER write outside the resolved output directory. The local
  `FileSystem` adapter enforces this; a `proptest` covers it.
- Templates may contain user-provided code — treat them as untrusted input.
  Path traversal, symlink escapes, and absolute paths in templates are
  rejected at plan-build time.

## 10. Non-goals are not features in waiting

`docs/NON_GOALS.md` lists ideas we **deliberately reject**. Before proposing
any of them, write an ADR explaining what trade-off changed. The PR template
includes a checkbox confirming the change is not on the non-goals list.

## 11. Where to look

- `docs/ARCHITECTURE.md` — diagram, exit codes, layering rules.
- `docs/NON_GOALS.md` — the rejection list.
- `docs/EXTRACT.md` — extract-pipeline contract.
- `docs/TEMPLATE_AUTHORING.md` — manifest, anchors, merges, injections.
- `docs/ADR/` — every load-bearing decision lives here.
- `PLAN.md` — phased delivery tracker (live).

## 12. Working with this repo (AI agents)

- Prefer **small, reviewable PRs**. One concept per PR.
- When unsure, read `docs/ADR/` for prior decisions before proposing a new
  approach.
- When adding a feature folder under a template, look at an existing
  feature first — anchor markers and merge conventions are stable.
- Run `scripts/check-agent-rules.sh` locally before pushing. It runs the
  forbidden-LLM-import grep, fmt, clippy, and tests.
