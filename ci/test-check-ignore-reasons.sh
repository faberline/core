#!/usr/bin/env bash
# Tests for ci/check-ignore-reasons.sh (issue #350).
# Usage: bash ci/test-check-ignore-reasons.sh
set -euo pipefail

script=ci/check-ignore-reasons.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# make_root <name> <rust source text>
# Creates $tmp/<name>/crates/x/tests/t.rs with that text and prints $tmp/<name>.
make_root() {
  local dir="$tmp/$1"
  mkdir -p "$dir/crates/x/tests"
  printf '%s\n' "$2" > "$dir/crates/x/tests/t.rs"
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

# 1. reason_ok: an ignored test with a reason passes.
root=$(make_root reason_ok '#[test]
#[ignore = "live kind cluster"]
fn a() {}')
expect reason_ok 0 'ignore reasons ok: 1 ignored tests' "$root"

# 2. bare_ignore_fails (negative): an ignore without a reason fails and
#    names the file and the line.
root=$(make_root bare_ignore_fails '#[ignore]
fn a() {}')
expect bare_ignore_fails 1 'has no reason' "$root"
expect bare_ignore_fails_location 1 'crates/x/tests/t.rs:1' "$root"

# 3. indented_bare_ignore_fails: an indented bare ignore on line 2.
root=$(make_root indented_bare_ignore_fails '#[test]
    #[ignore]
fn a() {}')
expect indented_bare_ignore_fails 1 't.rs:2' "$root"

# 4. empty_reason_fails: an empty reason string is no reason.
root=$(make_root empty_reason_fails '#[ignore = ""]
fn a() {}')
expect empty_reason_fails 1 'has no reason' "$root"

# 5. no_ignore_ok: no ignored test at all.
root=$(make_root no_ignore_ok 'fn a() {}')
expect no_ignore_ok 0 'ignore reasons ok: 0 ignored tests' "$root"

# 6. missing_crates_fails: the root has no crates/ directory.
mkdir -p "$tmp/empty"
expect missing_crates_fails 1 'missing' "$tmp/empty"

# 7. real_repo_ok: the real repository passes (no argument) with 15 ignored tests.
repo=$(cd "$(dirname "$0")/.." && pwd)
set +e
out=$(cd "$repo" && bash ci/check-ignore-reasons.sh 2>&1)
code=$?
set -e
if [ "$code" -eq 0 ] && grep -qF 'ignore reasons ok: 15 ignored tests' <<<"$out"; then
  echo "ok: real_repo_ok"
else
  echo "FAIL: real_repo_ok (exit $code): $out"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "check-ignore-reasons tests ok"
