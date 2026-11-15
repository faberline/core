#!/usr/bin/env bash
# Checks that every workspace crate declares its license (issue #364): a
# `license.workspace = true`, `license = "..."` or `license-file = "..."` line
# in its [package] table. When a crate inherits the workspace license, the
# workspace must have a license value, and for MIT the LICENSE file must match.
# Usage: bash ci/check-crate-license.sh [root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"

# table <file> <name>: prints the lines of the TOML table [<name>], from its
# header to the next line that starts with `[`.
table() {
  awk -v want="[$2]" '/^\[/ { p = ($0 == want); next } p { print }' "$1"
}

manifests=("$root"/crates/*/Cargo.toml)
if [ ! -f "${manifests[0]}" ]; then
  echo "FAIL: no crate manifests under $root/crates" >&2
  exit 1
fi

fail=0
count=0
inherit=0
for manifest in "${manifests[@]}"; do
  [ -f "$manifest" ] || continue
  count=$((count + 1))
  name=${manifest#"$root"/}
  pkg=$(table "$manifest" package)
  if grep -Eq '^license\.workspace[[:space:]]*=[[:space:]]*true' <<<"$pkg"; then
    inherit=1
  elif grep -Eq '^(license|license-file)[[:space:]]*=' <<<"$pkg"; then
    :
  else
    echo "FAIL: $name has no license field" >&2
    fail=1
  fi
done

wstable=$(table "$root/Cargo.toml" workspace.package 2>/dev/null || true)
ws=$(sed -n 's/^license[[:space:]]*=[[:space:]]*"\(.*\)".*$/\1/p' <<<"$wstable" | head -n 1 || true)

if [ "$inherit" -eq 1 ] && [ -z "$ws" ]; then
  echo "FAIL: license.workspace = true but [workspace.package] has no license" >&2
  exit 1
fi

if [ "$ws" = MIT ]; then
  first=$(head -n 1 "$root/LICENSE" 2>/dev/null || true)
  if [ "$first" != "MIT License" ]; then
    echo "FAIL: LICENSE does not match workspace license MIT" >&2
    exit 1
  fi
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "crate licenses ok: $count crates"
