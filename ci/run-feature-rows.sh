#!/usr/bin/env bash
# Runs every feature row of one crate, in table order, and prints one result
# line per row. Used by the test-feature-rows job in .github/workflows/ci.yml.
#
# Usage: bash ci/run-feature-rows.sh <package> [table]
#   The table defaults to ci/feature-rows.txt. Its rows are
#   <package>|<profile>|<args>; blank lines and lines starting with # are ignored.
#   FEATURE_ROWS_DRY_RUN=1 prints a DRY line per row and runs no cargo.
#
# Exit status: 2 when <package> is missing or the table has no row for it;
# 1 when a row failed; 0 otherwise. A failed row does not stop the others, so
# there is no `set -e` here.
set -uo pipefail

package="${1:-}"
if [ -z "$package" ]; then
  echo "usage: bash ci/run-feature-rows.sh <package> [table]" >&2
  exit 2
fi
table="${2:-ci/feature-rows.txt}"
if [ ! -f "$table" ]; then
  echo "run-feature-rows: no table at $table" >&2
  exit 2
fi

passed=0
failed=0
found=0
# Read the table on fd 3 so that cargo cannot consume the rows.
while IFS='|' read -r pkg profile args <&3; do
  case "$pkg" in
    '' | '#'*) continue ;;
  esac
  [ "$pkg" = "$package" ] || continue
  found=1
  if [ "${FEATURE_ROWS_DRY_RUN:-0}" = "1" ]; then
    echo "DRY $pkg $profile $args"
    continue
  fi
  echo "::group::$pkg $profile"
  row_ok=1
  # $args is unquoted on purpose: "--no-default-features --features x" must split.
  cargo test --locked -p "$pkg" $args -- --include-ignored || row_ok=0
  cargo clippy --locked -p "$pkg" $args --all-targets -- -D warnings || row_ok=0
  echo "::endgroup::"
  if [ "$row_ok" -eq 1 ]; then
    echo "ROW $pkg $profile PASS"
    passed=$((passed + 1))
  else
    echo "ROW $pkg $profile FAIL"
    failed=$((failed + 1))
  fi
done 3<"$table"

if [ "$found" -eq 0 ]; then
  echo "run-feature-rows: no row for $package in $table" >&2
  exit 2
fi
if [ "${FEATURE_ROWS_DRY_RUN:-0}" = "1" ]; then
  exit 0
fi

echo "ROWS $package: $passed passed, $failed failed"
if [ "$failed" -ne 0 ]; then
  exit 1
fi
exit 0
