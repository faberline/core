#!/usr/bin/env bash
# Run the kind rows of one or more e2e-service-k8s profiles, one after the
# other (issue #313). A row is cargo test + cargo clippy with the row's args,
# in the kind cluster of the job. A failed row does not stop the others:
# every row runs, one ROW line is printed for each, and the script fails at
# the end when one row failed.
# Usage: bash ci/run-kind-rows.sh <profile>...
#   the table is ci/kind-rows.txt next to this script.
# Env: KIND_ROWS_TABLE names another table (used by tests).
#      KIND_ROWS_DRY_RUN=1 prints DRY lines and runs no cargo.
#      GITHUB_STEP_SUMMARY, when set, gets one line per profile.
# No `set -e`: a failed row must not stop the job.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
table="${KIND_ROWS_TABLE:-$here/kind-rows.txt}"

if [ "$#" -eq 0 ]; then
  echo "usage: bash ci/run-kind-rows.sh <profile>..." >&2
  exit 2
fi

if [ ! -f "$table" ]; then
  echo "missing table: $table" >&2
  exit 2
fi

profiles=("$@")

# Resolve every profile before the first run.
args_list=()
for profile in "${profiles[@]}"; do
  args="$(awk -F'|' -v p="$profile" '$1==p {print substr($0, length(p)+2); found=1} END{exit !found}' "$table")" || {
    echo "no kind row for profile: $profile" >&2
    exit 2
  }
  args_list+=("$args")
done

if [ "${KIND_ROWS_DRY_RUN:-}" = "1" ]; then
  for i in "${!profiles[@]}"; do
    echo "DRY service-k8s ${profiles[$i]} ${args_list[$i]}"
  done
  exit 0
fi

passed=0
failed=0
for i in "${!profiles[@]}"; do
  profile="${profiles[$i]}"
  args="${args_list[$i]}"
  echo "::group::service-k8s $profile"
  rc=0
  cargo test --release --locked -p service-k8s $args -- --include-ignored --test-threads 1 < /dev/null || rc=1
  cargo clippy --locked -p service-k8s $args --all-targets -- -D warnings < /dev/null || rc=1
  echo "::endgroup::"
  if [ "$rc" -eq 0 ]; then
    status=PASS
    passed=$((passed + 1))
  else
    status=FAIL
    failed=$((failed + 1))
  fi
  echo "ROW service-k8s $profile $status"
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    echo "- service-k8s $profile: $status" >> "$GITHUB_STEP_SUMMARY"
  fi
done

echo "ROWS service-k8s: $passed passed, $failed failed"
if [ "$failed" -gt 0 ]; then
  exit 1
fi
exit 0
