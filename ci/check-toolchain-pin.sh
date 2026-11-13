#!/usr/bin/env bash
# Checks that rust-toolchain.toml pins a fixed Rust version and that every
# `toolchain:` line in .github/workflows/*.yml names the same version (issue #292).
# Usage: bash ci/check-toolchain-pin.sh [root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
toml="$root/rust-toolchain.toml"

if [ ! -f "$toml" ]; then
  echo "FAIL: missing $toml" >&2
  exit 1
fi

channel=$(sed -n 's/^channel = "\(.*\)"[[:space:]]*$/\1/p' "$toml" | head -n 1)
if [ -z "$channel" ]; then
  echo "FAIL: no channel = \"...\" line in $toml" >&2
  exit 1
fi
if ! [[ "$channel" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "FAIL: channel \"$channel\" is not a fixed N.N.N version" >&2
  exit 1
fi

fail=0
count=0
# One line per `toolchain:` line: "<file>:<line>" TAB "<version>".
while IFS=$'\t' read -r where value; do
  count=$((count + 1))
  if [ "$value" != "$channel" ]; then
    echo "FAIL: $where has toolchain: $value, expected $channel" >&2
    fail=1
  fi
done < <(
  for wf in "$root"/.github/workflows/*.yml; do
    [ -f "$wf" ] || continue
    awk -v f="$wf" '
      /^[[:space:]]*toolchain:[[:space:]]*[^[:space:]]+[[:space:]]*$/ {
        v = $0
        sub(/^[[:space:]]*toolchain:[[:space:]]*/, "", v)
        sub(/[[:space:]]*$/, "", v)
        print f ":" NR "\t" v
      }' "$wf"
  done
)

if [ "$count" -eq 0 ]; then
  echo "FAIL: no toolchain: line in $root/.github/workflows" >&2
  exit 1
fi
if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "toolchain pin ok: $channel"
