#!/usr/bin/env bash
# Tests for ci/upgrade-rollback-old-render.sh (#372). The positive case builds
# the old tag, so it is slow; the negative cases run first and fail fast.
# Usage: bash ci/test-upgrade-rollback-old-render.sh
set -euo pipefail

cd "$(dirname "$0")/.."

script=ci/upgrade-rollback-old-render.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# usage_without_out_file (negative): no arguments exits 2.
set +e
out=$(bash "$script" 2>&1)
code=$?
set -e
if [ "$code" -ne 2 ] || ! grep -qF 'usage: bash ci/upgrade-rollback-old-render.sh' <<<"$out"; then
  echo "FAIL: usage_without_out_file (exit $code): $out"
  failed=1
else
  echo "ok: usage_without_out_file"
fi

# missing_tag_fails (negative): an unknown tag exits 1, names the tag and writes nothing.
set +e
out=$(bash "$script" "$tmp/none.json" v9.9.9 2>&1)
code=$?
set -e
if [ "$code" -ne 1 ] || ! grep -qF 'FAIL: tag v9.9.9 not found' <<<"$out"; then
  echo "FAIL: missing_tag_fails (exit $code): $out"
  failed=1
elif [ -e "$tmp/none.json" ]; then
  echo "FAIL: missing_tag_fails (an output file was written)"
  failed=1
else
  echo "ok: missing_tag_fails"
fi

# old_render_has_the_probe_objects: the newest v0.* tag renders the two probe objects.
if UPGRADE_ROLLBACK_TARGET_DIR="$tmp/target" bash "$script" "$tmp/old.json" \
  && jq -e 'length == 2 and .[0].kind == "ServiceAccount" and .[0].metadata.name == "probe" and .[1].kind == "Service" and .[1].metadata.name == "probe-client"' "$tmp/old.json" >/dev/null; then
  echo "ok: old_render_has_the_probe_objects"
else
  echo "FAIL: old_render_has_the_probe_objects"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "upgrade-rollback old render tests ok"
