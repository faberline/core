#!/usr/bin/env bash
# Tests for ci/check-supply-chain-advisories.sh (issue #383).
# Usage: bash ci/test-check-supply-chain-advisories.sh (from the repository root)
set -euo pipefail

script=ci/check-supply-chain-advisories.sh
repo=$(cd "$(dirname "$0")/.." && pwd)
doc_rel=docs/operations/supply-chain-advisories.md
readme_rel=docs/operations/README.md
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# expect <name> <exit code> <output substring or -> <root>
# Runs the check on <root> and asserts the exit code and, when given,
# a substring of the combined output.
expect() {
  local name=$1 want_code=$2 want_text=$3 root=$4 out code
  set +e
  out=$(bash "$script" "$root" 2>&1)
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

# 1. real_doc_passes: the real repository passes (the doc and the README link).
expect real_doc_passes 0 'supply-chain advisories doc ok' "$repo"

# 2. missing_doc_fails (negative): no advisories document under the root.
root="$tmp/missing_doc_fails"
mkdir -p "$root/docs/operations"
cp "$repo/$readme_rel" "$root/$readme_rel"
expect missing_doc_fails 1 'missing' "$root"

# 3. missing_id_fails: the table lacks one of the 20 advisory ids.
root="$tmp/missing_id_fails"
mkdir -p "$root/docs/operations"
grep -v '^| RUSTSEC-2026-0386 |' "$repo/$doc_rel" > "$root/$doc_rel"
cp "$repo/$readme_rel" "$root/$readme_rel"
expect missing_id_fails 1 'RUSTSEC-2026-0386' "$root"

# 4. no_readme_link_fails: the README does not link the document.
root="$tmp/no_readme_link_fails"
mkdir -p "$root/docs/operations"
cp "$repo/$doc_rel" "$root/$doc_rel"
grep -v 'supply-chain-advisories.md' "$repo/$readme_rel" > "$root/$readme_rel"
expect no_readme_link_fails 1 'README' "$root"

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "check-supply-chain-advisories tests ok"
