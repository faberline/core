#!/usr/bin/env bash
# Runs cargo-deny and cargo-audit on a workspace (issue #353, point 7 of #340).
# The repository's deny.toml is the policy for cargo-deny; cargo-audit reads the lock file.
# Usage: bash ci/check-supply-chain.sh [dir]
# Both tools run; the script fails when either one fails.
set -uo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="${1:-$root}"

if [ ! -f "$dir/Cargo.toml" ] || [ ! -f "$dir/Cargo.lock" ]; then
  echo "FAIL: missing Cargo.toml or Cargo.lock in $dir" >&2
  exit 2
fi
for tool in cargo-deny cargo-audit; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "FAIL: missing tool $tool" >&2
    exit 2
  fi
done

cargo-deny --version
cargo-audit --version

rc=0
cargo deny --manifest-path "$dir/Cargo.toml" --config "$root/deny.toml" check || rc=1
cargo audit --file "$dir/Cargo.lock" || rc=1

if [ "$rc" -ne 0 ]; then
  echo "FAIL: supply chain check failed" >&2
  exit 1
fi
echo "supply chain ok"
