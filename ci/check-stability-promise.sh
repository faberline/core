#!/usr/bin/env bash
# Checks that docs/operations/stability-promise.md has a table row for every
# crate directory in crates/ (issue #347).
# Usage: bash ci/check-stability-promise.sh [root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
doc="$root/docs/operations/stability-promise.md"

if [ ! -f "$doc" ]; then
  echo "FAIL: missing $doc" >&2
  exit 1
fi

fail=0
count=0
for dir in "$root"/crates/*/; do
  [ -d "$dir" ] || continue
  name=$(basename "$dir")
  count=$((count + 1))
  # The row's first cell must be exactly `<name>`, so a longer crate name
  # does not satisfy a shorter one.
  if ! grep -q -- "^| \`${name}\` |" "$doc"; then
    echo "FAIL: crate $name has no row in docs/operations/stability-promise.md" >&2
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "stability promise ok: $count crates"
