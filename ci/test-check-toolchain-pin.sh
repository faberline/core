#!/usr/bin/env bash
# Tests for ci/check-toolchain-pin.sh (issue #292).
# Usage: bash ci/test-check-toolchain-pin.sh
set -euo pipefail

script=ci/check-toolchain-pin.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# make_root <name> <rust-toolchain.toml content | MISSING> <workflow yml content>
# Creates a fixture root under $tmp and prints its path.
make_root() {
  local dir="$tmp/$1"
  mkdir -p "$dir/.github/workflows"
  if [ "$2" != MISSING ]; then
    printf '%s\n' "$2" > "$dir/rust-toolchain.toml"
  fi
  printf '%s\n' "$3" > "$dir/.github/workflows/x.yml"
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

# 1. matching: the pin and both workflow lines agree.
root=$(make_root matching 'channel = "1.99.1"' 'toolchain: 1.99.1
toolchain: 1.99.1')
expect matching 0 'toolchain pin ok: 1.99.1' "$root"

# 2. stable_fails (negative): the old repo state, a workflow on `stable`.
root=$(make_root stable_fails 'channel = "1.99.1"' '      toolchain: stable')
expect stable_fails 1 'toolchain: stable' "$root"

# 3. mismatch_fails: one workflow line names another version; the output
#    names the file and the line.
root=$(make_root mismatch_fails 'channel = "1.99.1"' 'toolchain: 1.99.1
toolchain: 1.99.0')
expect mismatch_fails 1 'x.yml:2' "$root"

# 4. floating_channel_fails: the pin is not a fixed version.
root=$(make_root floating_channel_fails 'channel = "stable"' 'toolchain: stable')
expect floating_channel_fails 1 'not a fixed N.N.N version' "$root"

# 5. missing_file_fails: no rust-toolchain.toml.
root=$(make_root missing_file_fails MISSING 'toolchain: 1.99.1')
expect missing_file_fails 1 'missing' "$root"

# 6. no_toolchain_line_fails: the workflow has no toolchain: line.
root=$(make_root no_toolchain_line_fails 'channel = "1.99.1"' 'name: x')
expect no_toolchain_line_fails 1 'no toolchain:' "$root"

# 7. real_repo_ok: the real repository passes (no argument).
repo=$(cd "$(dirname "$0")/.." && pwd)
set +e
out=$(cd "$repo" && bash ci/check-toolchain-pin.sh 2>&1)
code=$?
set -e
if [ "$code" -eq 0 ] && grep -qF 'toolchain pin ok: 1.99.1' <<<"$out"; then
  echo "ok: real_repo_ok"
else
  echo "FAIL: real_repo_ok (exit $code): $out"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "check-toolchain-pin tests ok"
