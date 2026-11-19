#!/usr/bin/env bash
# Test for the gke-drill runner (ci/run-gke-drill.sh) and its workflow
# (.github/workflows/gke-drill.yml). A run of 0 tests is not a pass (core#531).
# Usage: bash ci/test-run-gke-drill.sh
set -euo pipefail

cd "$(dirname "$0")/.."

runner="ci/run-gke-drill.sh"
workflow=".github/workflows/gke-drill.yml"
aw_ref="f632d2715f8ef55caf48a3243ab1d72505ac1318"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"

# Fake cargo: `--list` prints $FAKE_LIST; any other call prints $FAKE_RUN and
# exits with $FAKE_RC.
cat > "$tmp/bin/cargo" <<'EOF'
#!/usr/bin/env bash
case " $* " in
  *" --list "*) cat "$FAKE_LIST" ;;
  *) cat "$FAKE_RUN"; exit "${FAKE_RC:-0}" ;;
esac
EOF
chmod +x "$tmp/bin/cargo"

# drill <list> <run> <rc>: runs the runner with the fake cargo.
# Sets rc, out (stdout) and err (stderr).
drill() {
  printf '%s\n' "$1" > "$tmp/list"
  printf '%s\n' "$2" > "$tmp/run"
  rc=0
  out="$(GKE_DRILL_CARGO="$tmp/bin/cargo" FAKE_LIST="$tmp/list" FAKE_RUN="$tmp/run" FAKE_RC="$3" \
    bash "$runner" 2>"$tmp/err")" || rc=$?
  err="$(cat "$tmp/err")"
}

two_tests='crate::a::smoke: test
crate::a::leader_loss: test'

# (a) zero_tests_fail: the filter lists no test. Exit 1 and say why.
drill "running 0 tests" "" 0
[ "$rc" -eq 1 ] || fail "zero_tests_fail: exit must be 1, was $rc"
grep -q 'matches 0 tests' <<<"$err" || fail "zero_tests_fail: stderr must contain 'matches 0 tests', was: $err"
echo "PASS: zero_tests_fail"

# (b) listed_and_passed_ok: 2 tests listed and 2 passed.
drill "$two_tests" "test result: ok. 2 passed; 0 failed; 0 ignored" 0
[ "$rc" -eq 0 ] || fail "listed_and_passed_ok: exit must be 0, was $rc"
echo "PASS: listed_and_passed_ok"

# (c) run_with_zero_passed_fails: 2 tests listed, but none passed.
drill "$two_tests" "test result: ok. 0 passed; 0 failed; 2 ignored" 0
[ "$rc" -eq 1 ] || fail "run_with_zero_passed_fails: exit must be 1, was $rc"
grep -q 'only 0 tests passed' <<<"$err" || fail "run_with_zero_passed_fails: stderr must contain 'only 0 tests passed', was: $err"
echo "PASS: run_with_zero_passed_fails"

# (d) failing_run_fails: the test run exits 101; the runner keeps that code.
drill "$two_tests" "test result: FAILED. 0 passed; 1 failed" 101
[ "$rc" -eq 101 ] || fail "failing_run_fails: exit must be 101, was $rc"
echo "PASS: failing_run_fails"

# (e) workflow_is_manual_only_and_guarded.
[ -f "$workflow" ] || fail "missing $workflow"
# Triggers are the lines that are not comments; the comment at the top names them.
code="$(grep -vE '^[[:space:]]*#' "$workflow")"
grep -qE '^  workflow_dispatch:[[:space:]]*$' <<<"$code" || fail "workflow must have workflow_dispatch:"
if grep -qE 'pull_request|push:' <<<"$code"; then
  fail "workflow must not run on pull_request or push"
fi
grep -q 'bash ci/run-gke-drill.sh' "$workflow" || fail "workflow must run bash ci/run-gke-drill.sh"
n_ref="$(grep -c "$aw_ref" "$workflow" || true)"
[ "$n_ref" -eq 2 ] || fail "workflow must name $aw_ref twice (uses and aw-ref), found $n_ref"
if grep -q '23808be0' "$workflow"; then
  fail "workflow must not name the old aw ref 23808be0"
fi
echo "PASS: workflow_is_manual_only_and_guarded"

echo "gke-drill runner ok"
