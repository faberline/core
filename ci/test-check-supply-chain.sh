#!/usr/bin/env bash
# Tests for ci/check-supply-chain.sh (issue #353).
# Needs cargo-deny 0.20.2, cargo-audit 0.23.0 and the advisory database (network).
# Usage: bash ci/test-check-supply-chain.sh
set -euo pipefail

cd "$(dirname "$0")/.."
script=ci/check-supply-chain.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# expect <name> <exit code> <output substring or -> <args...>
# Runs the check with <args> and asserts the exit code and, when given,
# a substring of the combined output.
expect() {
  local name=$1 want_code=$2 want_text=$3
  shift 3
  local out code
  set +e
  out=$(bash "$script" "$@" 2>&1)
  code=$?
  set -e
  if [ "$code" -ne "$want_code" ]; then
    echo "FAIL: $name (exit $code, want $want_code): $out"
    failed=1
    return
  fi
  if [ "$want_text" != "-" ] && ! grep -qF -- "$want_text" <<<"$out"; then
    echo "FAIL: $name (output lacks '$want_text'): $out"
    failed=1
    return
  fi
  echo "ok: $name"
}

# 1. negative: the seeded advisory fixture must fail the gate.
expect seeded_advisory_fails 1 RUSTSEC-2020-0071 ci/fixtures/supply-chain-advisory

# 2. the real workspace passes.
expect real_workspace_passes 0 "supply chain ok"

# 3. a root without Cargo.lock is a usage error.
mkdir -p "$tmp/no-lock"
printf '%s\n' '[package]' 'name = "x"' 'version = "0.0.0"' > "$tmp/no-lock/Cargo.toml"
expect missing_lock_fails 2 missing "$tmp/no-lock"

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "check-supply-chain tests ok"
