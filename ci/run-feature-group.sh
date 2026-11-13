#!/usr/bin/env bash
# Run the feature rows of several packages in one CI job, one package after
# the other (issue #312). Each package runs through ci/run-feature-rows.sh.
# A failed package does not stop the others: every package runs, one GROUP
# line is printed for each, and the script fails at the end when one failed.
# Usage: bash ci/run-feature-group.sh <package>...
# Env: RUN_FEATURE_ROWS names the per-package runner (default:
#   ci/run-feature-rows.sh next to this script).
#   GITHUB_STEP_SUMMARY, when set and not empty, gets one line per package.
# No `set -e`: a failed package must not stop the job.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
runner="${RUN_FEATURE_ROWS:-$here/run-feature-rows.sh}"

if [ "$#" -eq 0 ]; then
  echo "usage: bash ci/run-feature-group.sh <package>..." >&2
  exit 2
fi

passed=0
failed=0
for package in "$@"; do
  rc=0
  bash "$runner" "$package" || rc=$?
  if [ "$rc" = "0" ]; then
    echo "GROUP $package: PASS"
    passed=$((passed + 1))
    summary="- $package: PASS"
  else
    echo "GROUP $package: FAIL"
    failed=$((failed + 1))
    summary="- $package: FAIL"
  fi
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    echo "$summary" >> "$GITHUB_STEP_SUMMARY"
  fi
done

echo "GROUPS: $passed passed, $failed failed"
if [ "$failed" -gt 0 ]; then
  exit 1
fi
exit 0
