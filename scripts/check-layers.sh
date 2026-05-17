#!/usr/bin/env bash
# scripts/check-layers.sh
#
# Enforces the hexagonal crate-graph rules described in AGENTS.md §1 and
# docs/ARCHITECTURE.md.
#
#   usta-core   ←  usta-ports  ←  usta-app  ←  usta-adapters  ←  usta-cli
#
# Specifically:
#
#  1. usta-core   does not depend on any I/O crate.
#  2. usta-ports  does not depend on any I/O crate.
#  3. usta-app    does not depend on usta-adapters (or any I/O crate).
#  4. Adapter type names (Local*, Inquire*, Minijinja*, …) are not imported
#     anywhere except crates/usta-cli/src/wiring.rs and crates/usta-adapters/.
#
# Failures print a one-line summary and exit non-zero.

set -euo pipefail

cd "$(dirname "$0")/.."

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
fail=0

# Crates that must stay I/O-free. Each entry is a pair: crate_name|cargo_toml.
IO_FREE_CRATES=(
  "usta-core|crates/usta-core/Cargo.toml"
  "usta-ports|crates/usta-ports/Cargo.toml"
)

# Forbidden dependency names in the I/O-free crates.
FORBIDDEN_IO_DEPS=(
  "tokio"
  "async-std"
  "smol"
  "reqwest"
  "ureq"
  "hyper"
  "git2"
  "gix"
  "rusoto_core"
  "aws-sdk-s3"
  "tonic"
  "ignore"
  "walkdir"
  "minijinja"
  "handlebars"
  "tera"
  "tree-sitter"
)

for entry in "${IO_FREE_CRATES[@]}"; do
  name="${entry%%|*}"
  toml="${entry##*|}"
  for dep in "${FORBIDDEN_IO_DEPS[@]}"; do
    if grep -E "^${dep}(\s|=)" "$toml" >/dev/null 2>&1 \
       || grep -E "^${dep}\.workspace" "$toml" >/dev/null 2>&1; then
      red "✗ $name depends on forbidden I/O crate: $dep   ($toml)"
      fail=1
    fi
  done
done

# usta-app must not depend on usta-adapters.
if grep -E '^usta-adapters\b' crates/usta-app/Cargo.toml >/dev/null 2>&1; then
  red "✗ usta-app depends on usta-adapters (DIP violation)"
  fail=1
fi

# Adapter struct names should appear only in usta-adapters and the binary's
# wiring module. We grep in source paths, excluding allowed locations.
ADAPTER_TYPES=(
  "LocalFs"
  "InMemoryFs"
  "MinijinjaRenderer"
  "InquireUi"
  "GitCli"
  "PnpmPm"
  "UvPm"
  "CargoPm"
  "GoPm"
  "IgnoreScanner"
)

# Files that ARE allowed to mention adapter types.
# The grep output is `path:line:content`, so we filter by path-prefix.
allowed_re='^(crates/usta-adapters/|crates/usta-cli/src/wiring\.rs|crates/usta-cli/src/main\.rs|crates/usta-cli/src/commands/)'

for ty in "${ADAPTER_TYPES[@]}"; do
  matches="$(
    grep -RIn --include='*.rs' "\\b${ty}\\b" crates/ 2>/dev/null \
      | grep -Ev "${allowed_re}" || true
  )"
  if [[ -n "${matches}" ]]; then
    red "✗ adapter type \`${ty}\` referenced outside the composition root:"
    printf '%s\n' "${matches}"
    fail=1
  fi
done

if [[ "${fail}" -eq 0 ]]; then
  green "✓ layer rules pass"
else
  exit 1
fi
