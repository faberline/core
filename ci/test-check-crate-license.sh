#!/usr/bin/env bash
# Tests for ci/check-crate-license.sh (issue #364).
# Usage: bash ci/test-check-crate-license.sh
set -euo pipefail

script=ci/check-crate-license.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# make_root <name> <workspace license line | NONE>
# Creates a fixture root under $tmp with Cargo.toml, LICENSE and crates/.
# The caller adds crate manifests under crates/ afterwards.
make_root() {
  local dir="$tmp/$1"
  mkdir -p "$dir/crates"
  if [ "$2" = NONE ]; then
    printf '%s\n' '[workspace]' 'members = ["crates/*"]' '' '[workspace.package]' 'version = "0.1.0"' > "$dir/Cargo.toml"
  else
    printf '%s\n' '[workspace]' 'members = ["crates/*"]' '' '[workspace.package]' 'version = "0.1.0"' "$2" > "$dir/Cargo.toml"
  fi
  printf '%s\n' 'MIT License' '' 'Copyright (c) fixture' > "$dir/LICENSE"
  echo "$dir"
}

# add_crate <root> <name> <manifest body lines...>
# Writes crates/<name>/Cargo.toml with the given lines.
add_crate() {
  local root=$1 name=$2
  shift 2
  mkdir -p "$root/crates/$name"
  printf '%s\n' "$@" > "$root/crates/$name/Cargo.toml"
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

# 1. all_declared_ok: one crate inherits the license, one declares it.
root=$(make_root all_declared_ok 'license = "MIT"')
add_crate "$root" a '[package]' 'name = "a"' 'license.workspace = true'
add_crate "$root" b '[package]' 'name = "b"' 'license = "MIT"'
expect all_declared_ok 0 'crate licenses ok: 2 crates' "$root"

# 2. license_file_ok: a crate names a license file instead.
root=$(make_root license_file_ok 'license = "MIT"')
add_crate "$root" a '[package]' 'name = "a"' 'license-file = "../../LICENSE"'
expect license_file_ok 0 'crate licenses ok: 1 crates' "$root"

# 3. missing_fails (negative control): the old repo state, a [package] table
#    with no license field.
root=$(make_root missing_fails 'license = "MIT"')
add_crate "$root" a '[package]' 'name = "a"' 'version.workspace = true'
expect missing_fails 1 'crates/a/Cargo.toml has no license field' "$root"

# 4. license_in_dependencies_fails: a license line under [dependencies] does
#    not count as the crate's license.
root=$(make_root license_in_dependencies_fails 'license = "MIT"')
add_crate "$root" a '[package]' 'name = "a"' '' '[dependencies]' 'license = "x"'
expect license_in_dependencies_fails 1 'crates/a/Cargo.toml has no license field' "$root"

# 5. no_workspace_license_fails: a crate inherits, but the workspace has no
#    license value.
root=$(make_root no_workspace_license_fails NONE)
add_crate "$root" a '[package]' 'name = "a"' 'license.workspace = true'
expect no_workspace_license_fails 1 'no license' "$root"

# 6. license_text_mismatch_fails: workspace says MIT, LICENSE is another text.
root=$(make_root license_text_mismatch_fails 'license = "MIT"')
printf '%s\n' 'Apache License' > "$root/LICENSE"
add_crate "$root" a '[package]' 'name = "a"' 'license.workspace = true'
expect license_text_mismatch_fails 1 'does not match' "$root"

# 7. no_crates_fails: an empty crates/ directory.
root=$(make_root no_crates_fails 'license = "MIT"')
expect no_crates_fails 1 'no crate manifests' "$root"

# 8. real_repo_ok: the real repository passes (32 crates after the change).
repo=$(cd "$(dirname "$0")/.." && pwd)
set +e
out=$(cd "$repo" && bash ci/check-crate-license.sh 2>&1)
code=$?
set -e
if [ "$code" -eq 0 ] && grep -qF 'crate licenses ok: 32 crates' <<<"$out"; then
  echo "ok: real_repo_ok"
else
  echo "FAIL: real_repo_ok (exit $code): $out"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "check-crate-license tests ok"
