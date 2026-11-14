#!/usr/bin/env bash
# Semver check of the published library crates against a baseline (issue 348,
# part 2 of the 1.0.0 standard, faberline/aw#384).
#
# Usage: bash ci/semver-check.sh [<baseline-rev>]     (default: v0.4.14)
#
# Skipped, on purpose, with the reason:
#   e2e-support     publish = false, and it is not in the baseline release.
#   openapi-codegen publish = false (crates/openapi-codegen/Cargo.toml).
set -euo pipefail

baseline="${1:-v0.4.14}"

if ! command -v cargo-semver-checks >/dev/null 2>&1; then
  echo "FAIL: cargo-semver-checks is not installed" >&2
  exit 1
fi

cargo semver-checks --workspace --baseline-rev "$baseline" \
  --exclude e2e-support --exclude openapi-codegen
