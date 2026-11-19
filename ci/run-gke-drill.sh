#!/usr/bin/env bash
# Run the gke_drill tests on the shared GKE test cluster (core#531).
# A run of 0 tests is not a pass: the filter must match at least
# GKE_DRILL_MIN_TESTS tests, and that many must pass.
# Usage: bash ci/run-gke-drill.sh
# Env: GKE_DRILL_CARGO names the cargo command (default cargo; tests use a fake).
#      GKE_DRILL_MIN_TESTS is the least number of tests (default 1).
set -euo pipefail

cargo_cmd="${GKE_DRILL_CARGO:-cargo}"
min_tests="${GKE_DRILL_MIN_TESTS:-1}"

# Step A: the filter must list at least min_tests tests.
listed="$("$cargo_cmd" test -p service-k8s --test it gke_drill -- --ignored --list)"
n_listed="$(grep -c ': test$' <<<"$listed" || true)"
if [ "$n_listed" -lt "$min_tests" ]; then
  echo "FAIL: the gke_drill filter matches $n_listed tests, need at least $min_tests" >&2
  exit 1
fi
echo "gke_drill matches $n_listed tests"

# Step B: run them; keep the output to count the passed tests.
log="$(mktemp)"
rc=0
GKE_DRILL=1 "$cargo_cmd" test -p service-k8s --test it gke_drill -- --ignored --test-threads=1 2>&1 | tee "$log" || rc=$?
if [ "$rc" -ne 0 ]; then
  exit "$rc"
fi

# Step C: the passed tests must reach min_tests.
passed="$(sed -nE 's/^test result: ok\. ([0-9]+) passed.*/\1/p' "$log" | awk '{ s += $1 } END { print s + 0 }')"
if [ "$passed" -lt "$min_tests" ]; then
  echo "FAIL: only $passed tests passed, need at least $min_tests" >&2
  exit 1
fi
echo "gke_drill passed $passed tests"
exit 0
