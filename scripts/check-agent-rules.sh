#!/usr/bin/env bash
# scripts/check-agent-rules.sh
#
# Superset of check-layers.sh + check-forbidden-imports.sh, plus a few
# project-wide hygiene checks. This is the single command we ask
# contributors (human or AI) to run before pushing.

set -euo pipefail

cd "$(dirname "$0")/.."

red()    { printf '\033[31m%s\033[0m\n' "$*"; }
green()  { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }

step() { yellow "→ $*"; }

step "layer rules"
bash scripts/check-layers.sh

step "forbidden imports (no LLM SDKs)"
bash scripts/check-forbidden-imports.sh

step "AGENTS.md present"
test -f AGENTS.md || { red "✗ AGENTS.md missing"; exit 1; }

step "NON_GOALS.md present"
test -f docs/NON_GOALS.md || { red "✗ docs/NON_GOALS.md missing"; exit 1; }

step "ADR directory present"
test -d docs/ADR || { red "✗ docs/ADR/ missing"; exit 1; }

step "no TODO marker without an issue link"
# Heuristic: TODOs without "(#NN)" or a URL nearby are flagged. Soft-warn,
# not a hard failure — but we do report.
todos="$(grep -RIn --include='*.rs' --include='*.md' -E '\bTODO\b' \
           crates/ docs/ 2>/dev/null \
         | grep -Ev '(#[0-9]+|https?://)' || true)"
if [[ -n "${todos}" ]]; then
  yellow "⚠ TODOs without issue/URL reference (consider adding one):"
  printf '%s\n' "${todos}" | head -20
fi

step "cargo fmt --check"
cargo fmt --all -- --check

step "cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

step "cargo test"
cargo test --workspace --all-targets

green "✓ all agent-rule checks pass"
