#!/usr/bin/env bash
# Tests for ci/check-release-shape.sh (issue #225, part 2 of #223).
set -euo pipefail

check="ci/check-release-shape.sh"
release_src=".github/workflows/release.yml"
ci_src=".github/workflows/ci.yml"
tmp=$(mktemp -d)

# The check on the real files must pass.
if ! bash "$check" "$release_src" "$ci_src" >/dev/null; then
  echo "FAIL: the check rejects the real files" >&2
  exit 1
fi

# A mutation must make the check fail; a pass means the check misses that part.
expect_fail() {
  local name=$1 release_file=$2 ci_file=$3
  if bash "$check" "$release_file" "$ci_file" >/dev/null 2>&1; then
    echo "FAIL: mutation passed the check: $name" >&2
    exit 1
  fi
}

# 1. tag filter lost (the RC tag line is deleted)
m="$tmp/m1.yml"; cp "$release_src" "$m"
sed -i '/-rc\.\[0-9\]/d' "$m"
expect_fail "tag filter lost" "$m" "$ci_src"

# 2. preflight job lost
m="$tmp/m2.yml"; cp "$release_src" "$m"
sed -i 's/^  preflight:/  preflightx:/' "$m"
expect_fail "preflight job lost" "$m" "$ci_src"

# 3. performance job lost
m="$tmp/m3.yml"; cp "$release_src" "$m"
sed -i 's/^  perf-thresholds:/  perfx:/' "$m"
expect_fail "performance job lost" "$m" "$ci_src"

# 4. gate job lost
m="$tmp/m4.yml"; cp "$release_src" "$m"
sed -i 's/^  gate:/  gatex:/' "$m"
expect_fail "gate job lost" "$m" "$ci_src"

# 5. gate no longer rejects a skipped job
m="$tmp/m5.yml"; cp "$release_src" "$m"
sed -i "s/'skipped'/'unknown'/g" "$m"
expect_fail "gate no longer rejects skipped" "$m" "$ci_src"

# 6. a paths filter is added
m="$tmp/m6.yml"; cp "$release_src" "$m"
sed -i 's/^    tags:/    paths: [x]\n    tags:/' "$m"
expect_fail "paths filter added" "$m" "$ci_src"

# 7. ci.yml loses workflow_call
m="$tmp/m7.yml"; cp "$ci_src" "$m"
sed -i '/^  workflow_call:$/d' "$m"
expect_fail "workflow_call lost in ci.yml" "$release_src" "$m"

rm -rf "$tmp"
echo "check-release-shape tests ok"
