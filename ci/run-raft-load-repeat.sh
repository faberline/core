#!/usr/bin/env bash
# Repeat the ten raft-runtime tests of issue #556 under CPU load: LOAD_RUNS
# runs in a row, while LOAD_WORKERS busy-loop processes keep every core busy.
# A run passes when cargo exits 0 and prints "10 passed" (a renamed test must
# not pass silently). The first failed run stops the script at once.
# Usage: bash ci/run-raft-load-repeat.sh
# Env: LOAD_RUNS (default 20), LOAD_WORKERS (default nproc), CARGO (default cargo).
set -euo pipefail

LOAD_RUNS="${LOAD_RUNS:-20}"
LOAD_WORKERS="${LOAD_WORKERS:-$(nproc)}"
CARGO="${CARGO:-cargo}"

TESTS=(
  "publish_preflight::leader_retains_json_rejection_statuses_for_direct_host_and_registry"
  "read_index::follower_gets_the_leader_index_and_waits_for_apply"
  "read_index::leader_returns_commit_index_after_quorum_round"
  "read_index::partitioned_leader_times_out"
  "read_index::read_index_appends_no_log_entry"
  "read_index_peer::leader_read_index_endpoint_returns_the_index"
  "read_index_peer::registry_read_index_endpoint_routes_by_group_and_404s_unknown"
  "shutdown_idempotency::concurrent_shutdown_from_scratch_executes_once_and_returns_identical_phases"
  "shutdown_idempotency::repeat_shutdown_after_completion_emits_no_new_messages_and_returns_joined_report"
  "snapshot_preflight_release::preflight_does_not_hold_apply_and_stale_explicit_prefix_refuses"
)

# Build first, so that the compile is not starved by the load.
"$CARGO" test --locked -p raft-runtime --test it --no-run

PIDS=()
for ((w = 0; w < LOAD_WORKERS; w++)); do
  ( while :; do :; done ) &
  PIDS+=("$!")
  if [ "$w" -eq 0 ]; then
    # Stop the busy loops on pass, fail or Ctrl-C.
    trap 'kill "${PIDS[@]}" 2>/dev/null || true; wait 2>/dev/null || true' EXIT
  fi
done
echo "load: ${#PIDS[@]} busy-loop processes started"

for ((i = 1; i <= LOAD_RUNS; i++)); do
  set +e
  out="$("$CARGO" test --locked -p raft-runtime --test it -- --exact "${TESTS[@]}" 2>&1)"
  rc=$?
  set -e
  if [ "$rc" -eq 0 ] && grep -q 'test result: ok. 10 passed' <<<"$out"; then
    echo "run $i/$LOAD_RUNS: pass"
  else
    echo "run $i/$LOAD_RUNS: FAIL"
    printf '%s\n' "$out"
    exit 1
  fi
done

echo "all $LOAD_RUNS runs passed"
