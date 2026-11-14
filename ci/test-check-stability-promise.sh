#!/usr/bin/env bash
# Tests for ci/check-stability-promise.sh (issue #347).
# Usage: bash ci/test-check-stability-promise.sh
set -euo pipefail

script=ci/check-stability-promise.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# make_root <name> <stability-promise.md content | MISSING> <crate>...
# Creates a fixture root under $tmp with one crates/<crate>/ directory per
# crate name and prints its path.
make_root() {
  local dir="$tmp/$1" doc=$2
  shift 2
  mkdir -p "$dir/docs/operations"
  local crate
  for crate in "$@"; do
    mkdir -p "$dir/crates/$crate"
  done
  if [ "$doc" != MISSING ]; then
    printf '%s\n' "$doc" > "$dir/docs/operations/stability-promise.md"
  fi
  echo "$dir"
}

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

# 1. all_rows_ok: every crate has its row.
root=$(make_root all_rows_ok '# Stability promise
| Crate | Stable public surface | Wire format | On-disk format | CRD | Evidence |
|---|---|---|---|---|---|
| `alpha` | a | none | none | none | not measured |
| `beta` | b | none | none | none | not measured |' alpha beta)
expect all_rows_ok 0 'stability promise ok: 2 crates' "$root"

# 2. missing_row_fails (negative): beta has no row.
root=$(make_root missing_row_fails '# Stability promise
| `alpha` | a | none | none | none | not measured |' alpha beta)
expect missing_row_fails 1 'crate beta has no row' "$root"

# 3. prefix_not_enough_fails: a longer crate name must not satisfy a shorter one.
root=$(make_root prefix_not_enough_fails '# Stability promise
| `alpha-extra` | a | none | none | none | not measured |' alpha alpha-extra)
expect prefix_not_enough_fails 1 'crate alpha has no row' "$root"

# 4. missing_document_fails: no stability-promise.md at all.
root=$(make_root missing_document_fails MISSING alpha)
expect missing_document_fails 1 'missing' "$root"

# 5. real_repo_ok: the real repository has a row for each of its 31 crates.
repo=$(cd "$(dirname "$0")/.." && pwd)
set +e
out=$(cd "$repo" && bash ci/check-stability-promise.sh 2>&1)
code=$?
set -e
if [ "$code" -eq 0 ] && grep -qF 'stability promise ok: 31 crates' <<<"$out"; then
  echo "ok: real_repo_ok"
else
  echo "FAIL: real_repo_ok (exit $code): $out"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "check-stability-promise tests ok"
