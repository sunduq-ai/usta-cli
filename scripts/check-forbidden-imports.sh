#!/usr/bin/env bash
# scripts/check-forbidden-imports.sh
#
# Enforces the deterministic-extraction rule (AGENTS.md §3): no network LLM
# SDKs anywhere in the crate graph. This is a coarse grep-based check
# intended to fail loudly if anyone ever adds one of these crates.

set -euo pipefail

cd "$(dirname "$0")/.."

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }

# Banned crate names. If you add a hosted-LLM SDK to this project, you are
# breaking the headline guarantee. Don't.
BANNED=(
  "openai"
  "openai-api-rs"
  "async-openai"
  "anthropic-sdk"
  "anthropic"
  "claude-rs"
  "google-ai"
  "google-genai"
  "mistralai"
  "cohere-rust"
  "replicate-rs"
  "huggingface-hub"
)

fail=0

for dep in "${BANNED[@]}"; do
  hits="$(grep -RIn --include='Cargo.toml' -E "^[\"']?${dep}[\"']?\s*=" . 2>/dev/null || true)"
  if [[ -n "${hits}" ]]; then
    red "✗ banned LLM SDK detected: ${dep}"
    printf '%s\n' "${hits}"
    fail=1
  fi
done

# Also catch references in source code (someone might pull via a transitive
# trick or a vendored copy).
for dep in "${BANNED[@]}"; do
  hits="$(grep -RIn --include='*.rs' -E "use ${dep//-/_}::" crates/ 2>/dev/null || true)"
  if [[ -n "${hits}" ]]; then
    red "✗ banned LLM SDK imported: ${dep}"
    printf '%s\n' "${hits}"
    fail=1
  fi
done

if [[ "${fail}" -eq 0 ]]; then
  green "✓ no banned LLM SDKs"
else
  exit 1
fi
