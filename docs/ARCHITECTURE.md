# Architecture

## Crate graph

```
┌──────────────────────────────────────────────────────┐
│ usta-cli (binary, composition root)                  │
│   • clap subcommands                                 │
│   • wiring.rs is the only place adapters are named   │
└───────────────┬──────────────────────────────────────┘
                │ imports
       ┌────────┴────────┐
       ▼                 ▼
┌───────────────┐  ┌───────────────────────────────────┐
│ usta-app      │  │ usta-adapters                     │
│ (use cases)   │  │   • LocalFs / InMemoryFs          │
│   • generic   │  │   • MinijinjaRenderer             │
│     over ports│  │   • PnpmPm / UvPm / CargoPm / GoPm│
└──────┬────────┘  │   • GitCli                        │
       │           │   • IgnoreScanner                 │
       │           │   • TsSanitizer / PySanitizer …   │
       │           └─────────────┬─────────────────────┘
       │                         │
       ▼                         ▼
┌───────────────────────────────────────┐
│ usta-ports (trait definitions only)   │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│ usta-core (domain types, no I/O)      │
└───────────────────────────────────────┘
```

**The arrows are the dependency rule.** Reverse arrows are forbidden by the
compiler (because they would create cyclic crate deps) and by
`scripts/check-layers.sh` (which catches subtler leaks like importing
`usta-adapters` from `usta-app`).

## Why hexagonal, here?

We will scaffold many stacks. The CLI's concerns split cleanly into:

1. **What to scaffold** — pure data: templates, features, plans. (`core`)
2. **What I/O is needed** — abstract: filesystem, prompts, child processes,
   VCS, scanners. (`ports`)
3. **How a single use case orchestrates the abstract I/O** — pure logic over
   traits: scaffold, extract, list, update, add, verify. (`app`)
4. **Concrete I/O, one per backend** — opinionated, swappable. (`adapters`)
5. **Wiring + UX surface** — clap, indicatif, tracing. (`usta-cli` binary)

Each new template, package manager, language, or adapter is **additive**:
write a new module, register it in `wiring.rs`. The other 99% of the code
does not care.

## Exit codes

Stable across versions. Documented here so scripts wrapping `usta` can rely
on them.

| Code | Meaning |
|-----:|---------|
| 0    | Success. |
| 1    | Generic failure. Use the more-specific code where possible. |
| 2    | Argument parsing / usage error (`clap` default). |
| 3    | User cancelled (Ctrl-C, ESC at a prompt). |
| 10   | Domain error (invalid project name, unknown feature, conflict). |
| 11   | Manifest validation failure. |
| 12   | Renderer error. |
| 20   | Filesystem error (path traversal, permission denied). |
| 21   | Path-traversal violation specifically. |
| 30   | VCS error. |
| 31   | Package-manager error. |
| 40   | Update conflict requires manual resolution. |
| 41   | `verify` detected drift. |
| 50   | Extract: ambiguous classification with `--no-interactive`. |
| 64   | "Stub, not yet implemented" (P0 placeholders only; never shipped in releases). |

## Composition root

`crates/usta-cli/src/wiring.rs` is the **only** module allowed to mention
concrete adapter types. It builds use cases by passing concrete adapters
into generic constructors:

```rust
// illustrative — fully wired in P1
let fs = LocalFs::new(jail);
let renderer = MinijinjaRenderer::new();
let prompts = InquireUi::new();
let svc = ScaffoldService::new(fs, renderer, prompts);
```

If you find yourself naming an adapter type anywhere else, that is the
abstraction leak the layer rule is designed to catch.

## Where to add new things

| You want to add… | Add a… | In… |
|---|---|---|
| A new template (`go-service`, …) | folder + `template.toml` | `templates/<id>/` |
| A new package manager | adapter implementing `PackageManager` | `crates/usta-adapters/src/pkg/` |
| A new language sanitizer | adapter implementing `SourceSanitizer` | `crates/usta-adapters/src/sanitizers/` |
| A new stack detector | adapter implementing `StackDetector` | `crates/usta-adapters/src/detectors/` |
| A new use case (e.g. `usta lint`) | service + tests in `usta-app`, command stub in the binary | `crates/usta-app/src/<use_case>.rs` |
| A new port | trait in `usta-ports` + ADR | `crates/usta-ports/src/` |

Adding a port is the only one of these that should give you pause. Open an
ADR first.
