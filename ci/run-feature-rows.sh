#!/usr/bin/env bash
# Run all feature rows of one crate, one after the other (issue #241).
# A row is cargo test + cargo clippy with the row's args. A failed row
# does not stop the others: every row runs, one ROW line is printed for
# each, and the script fails at the end when one row failed.
# Usage: bash ci/run-feature-rows.sh <package> [table]
#   table defaults to ci/feature-rows.txt next to this script.
# Env: FEATURE_ROWS_DRY_RUN=1 prints DRY lines and runs no cargo.
# No `set -e`: a failed row must not stop the job.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
package="${1:-}"
table="${2:-$here/feature-rows.txt}"

if [ -z "$package" ]; then
  echo "usage: bash ci/run-feature-rows.sh <package> [table]" >&2
  exit 2
fi

if [ ! -f "$table" ]; then
  echo "missing table: $table" >&2
  exit 2
fi

rows=$(grep -vE '^(#|$)' "$table" | grep -E "^${package}\|" || true)
if [ -z "$rows" ]; then
  echo "no feature rows for package: $package" >&2
  exit 2
fi

if [ "${FEATURE_ROWS_DRY_RUN:-0}" = "1" ]; then
  while IFS='|' read -r pkg profile args; do
    echo "DRY $pkg $profile $args"
  done <<< "$rows"
  exit 0
fi

passed=0
failed=0
while IFS='|' read -r pkg profile args; do
  echo "::group::$pkg $profile"
  rc=0
  # $args is unquoted on purpose: it splits into cargo arguments.
  cargo test --locked -p "$pkg" $args -- --include-ignored < /dev/null || rc=1
  cargo clippy --locked -p "$pkg" $args --all-targets -- -D warnings < /dev/null || rc=1
  echo "::endgroup::"
  if [ "$rc" = "0" ]; then
    echo "ROW $pkg $profile PASS"
    passed=$((passed + 1))
  else
    echo "ROW $pkg $profile FAIL"
    failed=$((failed + 1))
  fi
done <<< "$rows"

echo "ROWS $package: $passed passed, $failed failed"
if [ "$failed" -gt 0 ]; then
  exit 1
fi
exit 0
