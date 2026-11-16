#!/usr/bin/env bash
# Checks that docs/operations/supply-chain-advisories.md lists all 20 cargo-deny
# advisories of the workspace, names the fixture and the MIT-0 licence question,
# and is linked from docs/operations/README.md (issue #383).
# Usage: bash ci/check-supply-chain-advisories.sh [root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
doc="$root/docs/operations/supply-chain-advisories.md"
readme="$root/docs/operations/README.md"

if [ ! -f "$doc" ]; then
  echo "FAIL: missing $doc" >&2
  exit 1
fi

fail=0
ids=(
  RUSTSEC-2026-0386 RUSTSEC-2026-0190 RUSTSEC-2026-0258 RUSTSEC-2026-0186
  RUSTSEC-2026-0204 RUSTSEC-2026-0285 RUSTSEC-2026-0346 RUSTSEC-2026-0348
  RUSTSEC-2026-0349 RUSTSEC-2026-0382 RUSTSEC-2026-0381 RUSTSEC-2026-0457
  RUSTSEC-2026-0461 RUSTSEC-2026-0462 RUSTSEC-2025-0134 RUSTSEC-2025-0141
  RUSTSEC-2024-0449 RUSTSEC-2025-0012 RUSTSEC-2024-0384 RUSTSEC-2024-0370
)
for id in "${ids[@]}"; do
  if ! grep -qF -- "| $id |" "$doc"; then
    echo "FAIL: advisory $id is not in the table" >&2
    fail=1
  fi
done

for text in RUSTSEC-2020-0071 borrow-or-share MIT-0; do
  if ! grep -qF -- "$text" "$doc"; then
    echo "FAIL: $text is not named in the document" >&2
    fail=1
  fi
done

if ! grep -qF -- "supply-chain-advisories.md" "$readme"; then
  echo "FAIL: the document is not linked from the README" >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "supply-chain advisories doc ok"
