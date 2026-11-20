#!/usr/bin/env bash
# Test for the raft-runtime load runner (ci/run-raft-load-repeat.sh, issue #556).
# A stub cargo stands in for the real one. Cases:
#   pass   20 runs pass: 20 pass lines, the load line, the final line.
#   fail3  run 3 fails: the runner stops there, no run 4.
#   zero   a run with 0 passed tests fails (the "10 passed" check is needed).
# Usage: bash ci/test-run-raft-load-repeat.sh
set -euo pipefail

cd "$(dirname "$0")/.."

runner="ci/run-raft-load-repeat.sh"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

[ -f "$runner" ] || fail "missing $runner"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/cargo" <<'EOF'
#!/usr/bin/env bash
case " $* " in
  *" --no-run "*) exit 0 ;;
esac
n=$(cat "$COUNT_FILE" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$COUNT_FILE"
case "$STUB_MODE" in
  pass)
    echo "test result: ok. 10 passed; 0 failed"
    exit 0
    ;;
  fail3)
    if [ "$n" -eq 3 ]; then
      echo "test result: FAILED. 9 passed; 1 failed"
      exit 101
    fi
    echo "test result: ok. 10 passed; 0 failed"
    exit 0
    ;;
  zero)
    echo "test result: ok. 0 passed; 0 failed"
    exit 0
    ;;
  *)
    echo "unknown STUB_MODE: $STUB_MODE" >&2
    exit 2
    ;;
esac
EOF
chmod +x "$tmp/cargo"

# run_case MODE RUNS OUTFILE: sets rc to the runner's exit code.
run_case() {
  rm -f "$tmp/count"
  rc=0
  STUB_MODE="$1" COUNT_FILE="$tmp/count" CARGO="$tmp/cargo" \
    LOAD_WORKERS=2 LOAD_RUNS="$2" bash "$runner" > "$3" 2>&1 || rc=$?
}

# (a) pass: 20 of 20 runs pass.
run_case pass 20 "$tmp/pass.out"
[ "$rc" -eq 0 ] || fail "pass case must exit 0, exit was $rc"
[ "$(grep -cE '^run [0-9]+/20: pass$' "$tmp/pass.out")" -eq 20 ] \
  || fail "pass case must print exactly 20 'run i/20: pass' lines"
grep -qx 'load: 2 busy-loop processes started' "$tmp/pass.out" \
  || fail "pass case must print 'load: 2 busy-loop processes started'"
[ "$(tail -n 1 "$tmp/pass.out")" = 'all 20 runs passed' ] \
  || fail "pass case last line must be 'all 20 runs passed'"

# (b) fail3: run 3 fails, the runner stops at once.
run_case fail3 5 "$tmp/fail3.out"
[ "$rc" -ne 0 ] || fail "fail3 case must exit non-zero"
grep -qx 'run 3/5: FAIL' "$tmp/fail3.out" || fail "fail3 case must print 'run 3/5: FAIL'"
[ "$(cat "$tmp/count")" -eq 3 ] || fail "fail3 case must stop at run 3, count was $(cat "$tmp/count")"
! grep -q 'run 4/5' "$tmp/fail3.out" || fail "fail3 case must not start run 4"

# (c) zero: a run that ran 0 tests must fail (negative control for the 10 passed check).
run_case zero 5 "$tmp/zero.out"
[ "$rc" -ne 0 ] || fail "zero case must exit non-zero"
grep -qx 'run 1/5: FAIL' "$tmp/zero.out" || fail "zero case must print 'run 1/5: FAIL'"

echo "raft load repeat ok"
