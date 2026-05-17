# Architecture

## Update (May 2026): single-crate collapse

`usta` shipped its 0.1.0 as a single Cargo crate. The hexagonal layout
described below is preserved as a **module hierarchy** under `src/`:

```
src/core/      (pure domain types)
src/ports/     (trait definitions)
src/app/       (use cases)
src/adapters/  (concrete implementations of ports)
src/commands/  (clap subcommand handlers)
src/main.rs    (clap entry point)
src/wiring.rs  (composition root — only place adapters meet use cases)
```

The principles, the dependency rule, and the "where to add new things"
guidance below all still apply — just read every "crate" as "module". The
trade-off: layer discipline is no longer compiler-enforced (Rust will
happily let `crate::app::scaffold` import `crate::adapters::fs::LocalFs`);
it is **enforced in code review**, aided by
`scripts/check-agent-rules.sh`. See ADR-0002 for the rationale.

## Module graph

```
┌──────────────────────────────────────────────────────────┐
│ binary (composition root)                                │
│   • src/main.rs        — clap entry point                │
│   • src/commands/      — one file per subcommand         │
│   • src/wiring.rs      — the only place adapter types    │
│                          are named outside `adapters/`   │
└───────────────┬──────────────────────────────────────────┘
                │ imports
       ┌────────┴────────┐
       ▼                 ▼
┌───────────────┐  ┌───────────────────────────────────────┐
│ crate::app    │  │ crate::adapters                       │
│ (use cases)   │  │   v0.1 shipping today:                │
│   • scaffold  │  │   • LocalFs / InMemoryFs              │
│   • add       │  │   • MinijinjaRenderer                 │
│   • update    │  │   • InquireUi / NoninteractiveUi      │
│   • verify    │  │   • IgnoreScanner                     │
│   • extract   │  │   • FilesystemTemplateSource          │
│   • list      │  │   • SystemClock                       │
└──────┬────────┘  └─────────────┬─────────────────────────┘
       │                         │
       ▼                         ▼
┌───────────────────────────────────────┐
│ crate::ports (trait definitions only) │
│   v0.1 wired:                         │
│   • FileSystem, TemplateRenderer,     │
│     PromptUi, RepoScanner,            │
│     TemplateSource, Clock             │
│   v0.2 defined but unwired:           │
│   • PackageManager, SourceSanitizer,  │
│     StackDetector, VcsClient,         │
│     Telemetry                         │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│ crate::core (domain types, no I/O)    │
│   • template, plan, snapshot, merge,  │
│     inject, resolver, project, paths  │
└───────────────────────────────────────┘
```

**The arrows are the dependency rule.** Reverse arrows are forbidden by
convention: `crate::core` and `crate::ports` must not import from
`crate::app` / `crate::adapters` / `crate::commands`, and `crate::app`
must not import from `crate::adapters`. Cargo no longer catches these
(the single-crate collapse traded that guarantee away); reviewers and
`scripts/check-agent-rules.sh` do.

## What ships in v0.1 vs. what the architecture is designed for

The hexagonal layout has been built out for a fuller v0.2 surface than
v0.1 ships. The gap is intentional — adding adapters later doesn't
require restructuring.

| Concern | Port (in `src/ports/`) | v0.1 adapter | Planned v0.2 adapter |
|---|---|---|---|
| Filesystem | `FileSystem` | `LocalFs`, `InMemoryFs` | — |
| Template rendering | `TemplateRenderer` | `MinijinjaRenderer` | — |
| Interactive prompts | `PromptUi` | `InquireUi`, `NoninteractiveUi` | — |
| Repo scanning | `RepoScanner` | `IgnoreScanner` | — |
| Template loading | `TemplateSource` | `FilesystemTemplateSource` | Git source, OCI source |
| Clock | `Clock` | `SystemClock` | — |
| Package manager | `PackageManager` | *none* | `PnpmPm`, `UvPm`, `CargoPm`, `GoPm` |
| Language sanitizer | `SourceSanitizer` | *none* | `TsSanitizer`, `PySanitizer`, … |
| Stack detection | `StackDetector` | *none* | `RustDetector`, `NodeDetector`, … |
| VCS | `VcsClient` | *none* (the binary shells out directly in `commands/new.rs`) | `GitCli` |
| Telemetry | `Telemetry` | *none* (no telemetry in v0.1) | opt-in local-only |

The v0.2 ports are kept compiling today (with `#![allow(dead_code)]` on
`src/ports/mod.rs`) so adding their adapters is purely additive.

## CLI surface vs. use-case layer

`crate::app` has 6 use cases. The CLI binary has 12 subcommands —
the difference is that 6 CLI subcommands are pure presentation / I/O and
live entirely under `src/commands/` with no use-case backing:

| Subcommand | Backed by use case in `crate::app` |
|---|---|
| `usta new` | `scaffold::ScaffoldService` |
| `usta add` | `add::add` |
| `usta update` | `update::update` |
| `usta verify` | `verify::verify` |
| `usta extract` | `extract::service` |
| `usta list templates` / `usta list features` | `list::*` |
| `usta doctor` | — pure binary |
| `usta search` | — pure binary |
| `usta install` | — pure binary |
| `usta completions` | — pure binary (uses `clap_complete`) |
| `usta self-update` | — pure binary |
| `usta schema` | — pure binary |

## Why hexagonal, here?

We will scaffold many stacks. The CLI's concerns split cleanly into:

1. **What to scaffold** — pure data: templates, features, plans. (`core`)
2. **What I/O is needed** — abstract: filesystem, prompts, child processes,
   VCS, scanners. (`ports`)
3. **How a single use case orchestrates the abstract I/O** — pure logic over
   traits: scaffold, extract, list, update, add, verify. (`app`)
4. **Concrete I/O, one per backend** — opinionated, swappable. (`adapters`)
5. **Wiring + UX surface** — clap, indicatif, tracing. (`commands` + `wiring`)

Each new template, package manager, language, or adapter is **additive**:
write a new module, register it in `wiring.rs`. The other 99% of the
code does not care.

## Exit codes

Stable across versions. Documented here so scripts wrapping `usta` can
rely on them. Codes marked **(v0.2)** are reserved — v0.1 returns generic
exit 1 for those failure modes (or clap's exit 2 for usage errors).
Future minor versions will emit the specific code without breaking the
existing exit-1 contract.

| Code | Meaning | Emitted in v0.1 |
|-----:|---------|:---------------:|
| 0    | Success. | ✓ |
| 1    | Generic failure (used for any error not specifically mapped below). | ✓ |
| 2    | Argument parsing / usage error (`clap` default). | ✓ |
| 3    | User cancelled (Ctrl-C, ESC at a prompt). | (v0.2) |
| 10   | Domain error (invalid project name, unknown feature, conflict). | (v0.2) |
| 11   | Manifest validation failure. | (v0.2) |
| 12   | Renderer error. | (v0.2) |
| 20   | Filesystem error (path traversal, permission denied). | (v0.2) |
| 21   | Path-traversal violation specifically. | (v0.2) |
| 30   | VCS error. | (v0.2) |
| 31   | Package-manager error. | (v0.2) |
| 40   | Update conflict requires manual resolution. | ✓ |
| 41   | `verify` detected drift. | ✓ |
| 50   | Extract: ambiguous classification with `--no-interactive`. | (v0.2) |
| 64   | "Stub, not yet implemented" (P0 placeholders only; never shipped in releases). | — |

## Composition root

`src/wiring.rs` is the **only** module outside `src/adapters/` allowed to
mention concrete adapter types. CLI subcommand handlers under
`src/commands/` are part of the binary (not the use-case layer) and may
also instantiate adapters directly — that exemption exists so simple
read-only commands like `usta list` don't need a wiring helper.

What the real `wiring.rs` looks like as of v0.1:

```rust
pub fn build_scaffold_service(
    project_root: PathBuf,
) -> ScaffoldService<LocalFs, MinijinjaRenderer, SystemClock> {
    let fs = LocalFs::new(project_root);
    let renderer = MinijinjaRenderer::new();
    let clock = SystemClock::new();
    ScaffoldService::new(fs, renderer, clock)
}

pub fn build_template_source(dir: PathBuf) -> FilesystemTemplateSource {
    FilesystemTemplateSource::new(dir)
}

pub fn build_prompt_ui(non_interactive: bool) -> Box<dyn PromptUi> {
    if non_interactive {
        Box::new(NoninteractiveUi)
    } else {
        Box::new(InquireUi::new())
    }
}
```

Use cases take their dependencies as generic parameters, not trait
objects — `Box<dyn Trait>` shows up only at the binary's composition
seam (here, for `PromptUi` because the choice is decided at runtime
based on `--yes`).

If you find yourself naming an adapter type anywhere else, that is the
abstraction leak the layer rule is designed to catch.

## Where to add new things

| You want to add… | Add a… | In… |
|---|---|---|
| A new template (`go-service`, …) | folder + `template.toml` | `templates/<id>/` |
| A new package manager | adapter implementing `PackageManager` | `src/adapters/<pkg_manager>.rs` (new file or new subdir) |
| A new language sanitizer | adapter implementing `SourceSanitizer` | `src/adapters/<sanitizer>.rs` (new file or new subdir) |
| A new stack detector | adapter implementing `StackDetector` | `src/adapters/<detector>.rs` (new file or new subdir) |
| A new use case (e.g. `usta lint`) | service + tests in `crate::app`, command handler + clap args in the binary | `src/app/<use_case>.rs` + `src/commands/<subcommand>.rs` |
| A new port | trait in `crate::ports` + ADR | `src/ports/<port>.rs` |
| A new CLI subcommand that doesn't need a use case (e.g. `usta doctor`) | clap args + handler | `src/commands/<subcommand>.rs` only |

Adding a port is the only one of these that should give you pause. Open
an ADR first.
