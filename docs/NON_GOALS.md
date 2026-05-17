# Non-Goals

These ideas are explicitly out of scope for `usta-cli`. Each was considered
and rejected for a stated trade-off. Reopening any of them requires an ADR
explaining what changed.

> **Why this file exists.** Non-goals quietly become goals when the original
> reasoning is forgotten. Pinning them down makes the project stable to
> contribute to: a contributor (human or AI) can read this list and avoid
> sinking effort into an idea that has already been weighed.

## Wasm plugin system

Massive complexity for little win. The trait-based adapter layer plus the
GitHub-topic template registry already cover ~95% of legitimate extension
needs at zero runtime cost. Sandboxing, ABI stability, and tooling for Wasm
plugins would dominate maintenance for a feature most users won't touch.

## Web playground / browser scaffolding

Cool demo, large maintenance tax (Wasm builds, sandboxed FS, hosted infra),
and orthogonal to the goal of saving tokens locally. The CLI is meant to run
on the user's machine where it has access to their package managers and VCS.

## Telemetry of any kind in v0.1

Adds a privacy surface area before there is enough usage to justify it.
Revisit only when concrete prioritization questions cannot be answered any
other way, and only as fully opt-in, anonymized, off by default. The
`Telemetry` port already exists in `usta-ports` so a future no-op-by-default
adapter can land without churn.

## Built-in TUI app explorer

`inquire` prompts cover the interactive surface. TUIs are an ongoing
maintenance commitment (terminal compatibility, redraw flicker, layout) that
rarely repays the effort for a tool people invoke a few times per project.

## Cloud sync of templates / hosted registry

GitHub already does this. We ship `usta install <gh-org>/<repo>` and a
topic-based registry; that is enough. We do not run a service, we do not
host an index, we do not require accounts.

## Network LLM calls in core flows

`extract` is deterministic. The `SourceClassifier` port permits a future
local-only adapter (e.g. Ollama on `127.0.0.1`) — never a hosted LLM. CI
greps for forbidden imports (`openai`, `anthropic-sdk`, `genai`, …) and
fails on any match.

## Auto-formatting / opinionated code rewrites beyond template scope

We render templates and run user-declared post-hooks (e.g. `prettier`,
`ruff format`). We do not editorialize generated code beyond what the
template author specified. Generated apps are the template author's
responsibility.

## Lock-in to a single ecosystem (npm, pip, …)

The CLI is a single static binary precisely so it stays neutral across the
stacks it scaffolds. We will not publish a `npm` package, a `pip` package,
or a `go install`-able mirror. `cargo install`, GitHub Releases, and a
Homebrew tap are the only distribution channels.

## Replacing existing scaffolders

`copier`, `cookiecutter`, `degit`, `create-*`, and Nx generators all have
their place. `usta` is justified by the **combination** of: multi-stack
single binary + deterministic `extract` + post-hoc `add`/`update`. Where one
of those isn't load-bearing, the existing tools are usually the right
answer; we will say so in the README.

## Hidden state outside `.usta/`

The only state we write into a generated project is under `.usta/`. We do
not edit `.git/`, we do not write home-directory state per project, we do
not patch `~/.zshrc`. If a user deletes `.usta/`, `usta update` falls back
to the same flow as a fresh `usta new` against the current state.
