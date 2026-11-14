#!/usr/bin/env bash
# Tests for ci/check-ci-shape.sh (issue #150).
# Usage: bash ci/test-check-ci-shape.sh
set -euo pipefail

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Positive case: the real ci.yml must pass.
bash ci/check-ci-shape.sh .github/workflows/ci.yml

# expect_reject <label> <if-value>
# Puts <if-value> on the gate job's `if:` line and expects the shape check
# to reject it with the gate message.
expect_reject() {
  local label=$1 value=$2 err code
  sed "/^  gate:\$/,/^    needs:/ s#^    if: always()\$#    if: $value#" \
    .github/workflows/ci.yml > "$tmp/ci.yml"
  if cmp -s .github/workflows/ci.yml "$tmp/ci.yml"; then
    echo "FAIL: $label (sed changed nothing)"
    exit 1
  fi
  set +e
  err=$(bash ci/check-ci-shape.sh "$tmp/ci.yml" 2>&1 >/dev/null)
  code=$?
  set -e
  if [ "$code" -eq 0 ] || ! grep -q 'gate does not have if: always()' <<<"$err"; then
    echo "FAIL: $label"
    exit 1
  fi
}

expect_reject and-false 'always() \&\& false'
expect_reject or-always 'success() || always()'
expect_reject failure 'failure()'
expect_reject success 'success()'

# expect_shape_reject <label> <message>
# Expects the shape check to reject the already written "$tmp/ci.yml" with <message>.
expect_shape_reject() {
  local label=$1 message=$2 err code
  if cmp -s .github/workflows/ci.yml "$tmp/ci.yml"; then
    echo "FAIL: $label (sed changed nothing)"
    exit 1
  fi
  set +e
  err=$(bash ci/check-ci-shape.sh "$tmp/ci.yml" 2>&1 >/dev/null)
  code=$?
  set -e
  if [ "$code" -eq 0 ] || ! grep -qF "$message" <<<"$err"; then
    echo "FAIL: $label"
    exit 1
  fi
}

sed '/^  gate:$/,/^    needs:/ s/ test-select,//' .github/workflows/ci.yml > "$tmp/ci.yml"
expect_shape_reject gate-without-select 'gate does not need test-select'

sed 's/name: "test: select jobs"/name: "select jobs"/' .github/workflows/ci.yml > "$tmp/ci.yml"
expect_shape_reject select-name 'job test-select has no test: display name'

sed '/^  test-select:$/,/^    runs-on:/ s/^    needs: lint$/    needs: []/' .github/workflows/ci.yml > "$tmp/ci.yml"
expect_shape_reject select-needs 'test-select does not need lint'

echo "check-ci-shape tests ok"
