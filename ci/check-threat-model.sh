#!/usr/bin/env bash
# Checks that docs/operations/threat-model.md has its six sections, names the
# independent review, cites only paths that exist, and is linked from
# docs/operations/README.md (issue #354, point 7 of #340).
# Usage: bash ci/check-threat-model.sh [root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
doc="$root/docs/operations/threat-model.md"
readme="$root/docs/operations/README.md"

if [ ! -f "$doc" ]; then
  echo "FAIL: missing $doc" >&2
  exit 1
fi

fail=0
headings=(
  "## Auth"
  "## TLS"
  "## Audit"
  "## Secrets"
  "## Supply chain of the git-tag dependency model"
  "## Not applicable to core"
)
for heading in "${headings[@]}"; do
  if ! grep -qxF -- "$heading" "$doc"; then
    echo "FAIL: section '$heading' is missing" >&2
    fail=1
  fi
done

if ! grep -qi "independent review" "$doc"; then
  echo "FAIL: the independent review is not named" >&2
  fail=1
fi

# Cited paths: a backticked path under crates/, docs/, ci/, vendor/ or .github/,
# or deny.toml or Cargo.lock. One trailing slash is stripped.
paths=$(grep -oE '`((crates|docs|ci|vendor|\.github)/[A-Za-z0-9_./-]+|deny\.toml|Cargo\.lock)`' "$doc" \
  | tr -d '`' | sed 's#/$##' | sort -u || true)
count=0
while IFS= read -r path; do
  [ -n "$path" ] || continue
  count=$((count + 1))
  if ! test -e "$root/$path"; then
    echo "FAIL: cited path does not exist: $path" >&2
    fail=1
  fi
done <<<"$paths"

if ! grep -qF 'threat-model.md' "$readme" 2>/dev/null; then
  echo "FAIL: docs/operations/README.md does not link threat-model.md" >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "threat model ok: 6 sections, $count cited paths"
