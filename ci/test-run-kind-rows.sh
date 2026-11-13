#!/usr/bin/env bash
# Test for the kind-row table (ci/kind-rows.txt), its runner
# (ci/run-kind-rows.sh) and the e2e-service-k8s matrix in ci.yml.
# Proves that the 9 kind profiles run in exactly one of the 3 groups (issue #313).
# Usage: bash ci/test-run-kind-rows.sh
set -euo pipefail

cd "$(dirname "$0")/.."

table="ci/kind-rows.txt"
runner="ci/run-kind-rows.sh"
ci_file=".github/workflows/ci.yml"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

# Frozen profile|args lines of the 9 kind rows (issue #313).
frozen_rows() {
  cat <<'EOF'
default|
all-features|--all-features
no-default|--no-default-features
feature:certificate|--no-default-features --features certificate
feature:controller|--no-default-features --features controller
feature:full|--no-default-features --features full
feature:gcp-cas|--no-default-features --features gcp-cas
feature:gcp-cas-client|--no-default-features --features gcp-cas-client
feature:render-only|--no-default-features --features render-only
EOF
}

all_profiles="default all-features no-default feature:certificate feature:controller feature:full feature:gcp-cas feature:gcp-cas-client feature:render-only"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"

# (a) Table: exactly the 9 frozen rows.
[ -f "$table" ] || fail "missing table: $table"
data_lines="$(grep -vE '^(#|$)' "$table" || true)"
count="$(printf '%s\n' "$data_lines" | grep -c . || true)"
[ "$count" -eq 9 ] || fail "$table must have 9 data lines, found $count"
diff <(frozen_rows | sort) <(printf '%s\n' "$data_lines" | sort) >/dev/null \
  || fail "$table lines differ from the 9 kind rows"

# (b) Matrix: 3 include rows with group names; each of the 9 profiles exactly once.
[ -f "$ci_file" ] || fail "missing $ci_file"
job="$(awk '/^  e2e-service-k8s:/{f=1; next} /^  [a-z0-9-]+:$/{f=0} f' "$ci_file")"
include_lines="$(printf '%s\n' "$job" | grep -E '^[[:space:]]*- \{ group: ' || true)"
n_groups="$(printf '%s\n' "$include_lines" | grep -c . || true)"
[ "$n_groups" -eq 3 ] || fail "e2e-service-k8s must have 3 include rows with group:, found $n_groups"
groups="$(printf '%s\n' "$include_lines" | sed -E 's/.*group: ([^,]*),.*/\1/' | sort | tr '\n' ' ')"
[ "$groups" = "group-a group-b group-c " ] \
  || fail "groups of e2e-service-k8s must be group-a group-b group-c, found: $groups"
profiles="$(printf '%s\n' "$include_lines" | sed -E 's/.*profiles: "([^"]*)".*/\1/' | tr ' ' '\n' | grep -v '^$' | sort || true)"
dups="$(printf '%s\n' "$profiles" | uniq -d)"
[ -z "$dups" ] || fail "matrix of e2e-service-k8s must hold each of the 9 profiles exactly once (duplicates: $dups)"
expected="$(frozen_rows | cut -d'|' -f1 | sort)"
[ "$profiles" = "$expected" ] \
  || fail "matrix of e2e-service-k8s must hold each of the 9 profiles exactly once"

# (c) Dry run prints DRY lines and runs no cargo.
out="$(KIND_ROWS_DRY_RUN=1 bash "$runner" default feature:full)" \
  || fail "dry run of default feature:full must exit 0"
expected_out="$(printf 'DRY service-k8s default \nDRY service-k8s feature:full --no-default-features --features full')"
[ "$out" = "$expected_out" ] || fail "dry run output differs: $out"
n_dry="$(KIND_ROWS_DRY_RUN=1 bash "$runner" $all_profiles | grep -c '^DRY service-k8s ' || true)"
[ "$n_dry" -eq 9 ] || fail "dry run of all 9 profiles must print 9 DRY lines, found $n_dry"

# (d) A failed profile does not stop the others (negative control).
cat > "$tmp/bin/cargo" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"--features certificate"*) exit 1 ;;
esac
exit 0
EOF
chmod +x "$tmp/bin/cargo"
rc=0
out="$(PATH="$tmp/bin:$PATH" GITHUB_STEP_SUMMARY="$tmp/summary.md" bash "$runner" default feature:certificate feature:controller 2>&1)" || rc=$?
[ "$rc" -eq 1 ] || fail "a failed profile must make the runner exit 1, exit was $rc"
grep -qx 'ROW service-k8s default PASS' <<<"$out" || fail "missing line: ROW service-k8s default PASS"
grep -qx 'ROW service-k8s feature:certificate FAIL' <<<"$out" || fail "missing line: ROW service-k8s feature:certificate FAIL"
grep -qx 'ROW service-k8s feature:controller PASS' <<<"$out" || fail "the row after a failure must still run: ROW service-k8s feature:controller PASS"
last="$(printf '%s\n' "$out" | tail -n 1)"
[ "$last" = 'ROWS service-k8s: 2 passed, 1 failed' ] || fail "last line must be 'ROWS service-k8s: 2 passed, 1 failed', was: $last"
grep -qx -- '- service-k8s default: PASS' "$tmp/summary.md" || fail "step summary misses the default line"
grep -qx -- '- service-k8s feature:certificate: FAIL' "$tmp/summary.md" || fail "step summary misses the certificate line"
grep -qx -- '- service-k8s feature:controller: PASS' "$tmp/summary.md" || fail "step summary misses the controller line"

# (e) All pass: exit 0.
cat > "$tmp/bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
rc=0
out="$(PATH="$tmp/bin:$PATH" bash "$runner" default all-features 2>&1)" || rc=$?
[ "$rc" -eq 0 ] || fail "all rows passing must exit 0, exit was $rc"
last="$(printf '%s\n' "$out" | tail -n 1)"
[ "$last" = 'ROWS service-k8s: 2 passed, 0 failed' ] || fail "last line must be 'ROWS service-k8s: 2 passed, 0 failed', was: $last"

# (f) Errors: exit 2 before any run.
rc=0
bash "$runner" >/dev/null 2>&1 || rc=$?
[ "$rc" -eq 2 ] || fail "no argument must exit 2, exit was $rc"
rc=0
out="$(bash "$runner" no-such-profile 2>&1)" || rc=$?
[ "$rc" -eq 2 ] || fail "an unknown profile must exit 2, exit was $rc"
if grep -q '^ROW ' <<<"$out"; then fail "an unknown profile must print no ROW line"; fi
marker="$tmp/cargo-called"
cat > "$tmp/bin/cargo" <<EOF
#!/usr/bin/env bash
touch "$marker"
exit 0
EOF
rc=0
PATH="$tmp/bin:$PATH" bash "$runner" default no-such-profile >/dev/null 2>&1 || rc=$?
[ "$rc" -eq 2 ] || fail "'default no-such-profile' must exit 2, exit was $rc"
[ ! -e "$marker" ] || fail "cargo must not run when one profile is unknown"

echo "kind rows ok"
