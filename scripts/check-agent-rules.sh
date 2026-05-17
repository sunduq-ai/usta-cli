#!/usr/bin/env bash
# scripts/check-agent-rules.sh
#
# Single command we ask contributors (human or AI) to run before pushing.
# A few project-wide hygiene checks + fmt + clippy + test.
#
# The crate graph used to be enforced by a separate `check-layers.sh` that
# verified inter-crate dependencies; the workspace has since been collapsed
# to a single crate, so layer rules are now a code-review responsibility
# rather than a Cargo-enforced one. See AGENTS.md §1.

set -euo pipefail

cd "$(dirname "$0")/.."

red()    { printf '\033[31m%s\033[0m\n' "$*"; }
green()  { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }

step() { yellow "→ $*"; }

step "AGENTS.md present"
test -f AGENTS.md || { red "✗ AGENTS.md missing"; exit 1; }

step "NON_GOALS.md present"
test -f docs/NON_GOALS.md || { red "✗ docs/NON_GOALS.md missing"; exit 1; }

step "ADR directory present"
test -d docs/ADR || { red "✗ docs/ADR/ missing"; exit 1; }

step "no LLM SDK imports in source"
# Defensive grep: there must be no network LLM client anywhere in the
# engine. See docs/NON_GOALS.md.
forbidden_imports="$(grep -RIn --include='*.rs' \
  -E '^\s*use\s+(anthropic|openai|google_genai|ollama|llama_cpp|tiktoken_rs)' \
  src/ 2>/dev/null || true)"
if [[ -n "${forbidden_imports}" ]]; then
  red "✗ forbidden LLM SDK imports found:"
  printf '%s\n' "${forbidden_imports}"
  exit 1
fi

step "no TODO marker without an issue link"
# Heuristic: TODOs without "(#NN)" or a URL nearby are flagged. Soft-warn,
# not a hard failure — but we do report.
todos="$(grep -RIn --include='*.rs' --include='*.md' -E '\bTODO\b' \
           src/ docs/ 2>/dev/null \
         | grep -Ev '(#[0-9]+|https?://)' || true)"
if [[ -n "${todos}" ]]; then
  yellow "⚠ TODOs without issue/URL reference (consider adding one):"
  printf '%s\n' "${todos}" | head -20
fi

step "cargo fmt --check"
cargo fmt --all -- --check

step "cargo clippy"
cargo clippy --all-targets -- -D warnings

step "cargo test"
cargo test --all-targets

green "✓ all agent-rule checks pass"
