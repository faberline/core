#!/usr/bin/env bash
# Test for ci/semver-check.sh (issue 348): on a fixture crate, a removed public
# item must fail cargo-semver-checks, and an unchanged crate must pass.
# Usage: bash ci/test-semver-check.sh
set -euo pipefail

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

if ! command -v cargo-semver-checks >/dev/null 2>&1; then
  echo "FAIL: cargo-semver-checks is not installed" >&2
  exit 1
fi

# Fixture: a crate at 1.0.0 with two public functions, tagged v1.0.0.
fx="$tmp/fixture"
mkdir -p "$fx/src"
cat > "$fx/Cargo.toml" <<'EOF'
[package]
name = "semver-fixture"
version = "1.0.0"
edition = "2021"

[dependencies]
EOF
printf '%s\n' 'pub fn kept() {}' 'pub fn removed() {}' > "$fx/src/lib.rs"
cd "$fx"
# Keep cargo-semver-checks' baseline clone out of the fixture, or cargo sees two
# manifests named semver-fixture under the fixture root.
export CARGO_TARGET_DIR="$tmp/target"
git init -q
git config user.email ci@example.invalid
git config user.name ci
git add -A
git commit -q -m base
git tag v1.0.0

# removed_item_fails (negative): drop `removed`, bump to 1.0.1. The check must
# fail and name the removed item.
printf '%s\n' 'pub fn kept() {}' > src/lib.rs
sed -i 's/^version = "1.0.0"/version = "1.0.1"/' Cargo.toml
set +e
out=$(cargo semver-checks --baseline-rev v1.0.0 2>&1)
code=$?
set -e
if [ "$code" -ne 0 ] && grep -q 'removed' <<<"$out"; then
  echo "ok: removed_item_fails"
else
  echo "FAIL: removed_item_fails (exit $code): $out"
  failed=1
fi

# unchanged_ok (positive): restore the public API, keep 1.0.1. The check must pass.
git checkout -q -- src/lib.rs
set +e
out=$(cargo semver-checks --baseline-rev v1.0.0 2>&1)
code=$?
set -e
if [ "$code" -eq 0 ]; then
  echo "ok: unchanged_ok"
else
  echo "FAIL: unchanged_ok (exit $code): $out"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "semver-check tests ok"
