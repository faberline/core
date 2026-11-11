#!/usr/bin/env bash
# Checks the feature-row table (ci/feature-rows.txt), its runner
# (ci/run-feature-rows.sh) and the test-feature-rows matrix in ci.yml.
# Usage: bash ci/test-feature-rows.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

table=ci/feature-rows.txt
runner=ci/run-feature-rows.sh
workflow=.github/workflows/ci.yml

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

[ -f "$table" ] || fail "missing $table"
[ -f "$runner" ] || fail "missing $runner"
[ -f "$workflow" ] || fail "missing $workflow"

# The 31 rows as they run today (copied from the test-feature-rows matrix).
frozen=$(cat <<'EOF'
index-text|all-features|--all-features
index-text|feature:jieba|--no-default-features --features jieba
index-text|no-default|--no-default-features
service-auth|all-features|--all-features
service-auth|feature:k8s|--no-default-features --features k8s
service-auth|no-default|--no-default-features
service-backup|all-features|--all-features
service-backup|feature:s3|--no-default-features --features s3
service-backup|no-default|--no-default-features
service-http|all-features|--all-features
service-http|feature:otlp|--no-default-features --features otlp
service-http|no-default|--no-default-features
service-observability|all-features|--all-features
service-observability|feature:otlp|--no-default-features --features otlp
service-observability|no-default|--no-default-features
storage-object|all-features|--all-features
storage-object|feature:gcs|--no-default-features --features gcs
storage-object|feature:s3|--no-default-features --features s3
storage-object|no-default|--no-default-features
transport-h2c|all-features|--all-features
transport-h2c|feature:server|--no-default-features --features server
transport-h2c|no-default|--no-default-features
ui-runtime|all-features|--all-features
ui-runtime|feature:debug|--no-default-features --features debug
ui-runtime|feature:serde_json|--no-default-features --features serde_json
ui-runtime|no-default|--no-default-features
cli-std|all-features|--all-features
cli-std|feature:k8s|--no-default-features --features k8s
cli-std|feature:online|--no-default-features --features online
cli-std|feature:registry|--no-default-features --features registry
cli-std|no-default|--no-default-features
EOF
)
crates=$(printf '%s\n' index-text service-auth service-backup service-http \
  service-observability storage-object transport-h2c ui-runtime cli-std | sort)

# Data rows of the table: not empty, not a comment.
rows=$(grep -vE '^(#|$)' "$table" || true)

# (a) exactly 31 rows, and the same package|profile pairs as the frozen list.
[ "$(grep -c . <<<"$rows")" -eq 31 ] || fail "table has $(grep -c . <<<"$rows") rows, expected 31"
diff <(cut -d'|' -f1,2 <<<"$rows" | sort) <(cut -d'|' -f1,2 <<<"$frozen" | sort) \
  || fail "(a) package|profile pairs differ from the frozen 31 rows"

# (b) the whole table, args included, equals the frozen table.
diff <(sort <<<"$rows") <(sort <<<"$frozen") || fail "(b) table rows or args differ from the frozen table"

# (c) the distinct packages are the 9 crates, and the matrix of
# test-feature-rows in ci.yml names exactly those 9 packages.
diff <(cut -d'|' -f1 <<<"$rows" | sort -u) <(sort <<<"$crates") \
  || fail "(c) packages in the table are not the 9 crates"
matrix_pkgs=$(awk '/^  test-feature-rows:$/ {on=1; next}
  on && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {on=0}
  on {print}' "$workflow" \
  | sed -n 's/^ *- { package: \([^ ]*\) }$/\1/p')
[ "$(grep -c . <<<"$matrix_pkgs")" -eq 9 ] || fail "(c) test-feature-rows matrix has $(grep -c . <<<"$matrix_pkgs") package lines, expected 9"
diff <(sort <<<"$matrix_pkgs") <(sort <<<"$crates") \
  || fail "(c) ci.yml matrix packages are not the 9 crates"

# (d) the dry run prints one DRY line per row of each package; the sum is 31.
total=0
for pkg in $crates; do
  want=$(cut -d'|' -f1 <<<"$rows" | grep -cx "$pkg" || true)
  got=$(FEATURE_ROWS_DRY_RUN=1 bash "$runner" "$pkg" | grep -c '^DRY ' || true)
  [ "$got" -eq "$want" ] || fail "(d) $pkg: dry run printed $got DRY lines, table has $want rows"
  total=$((total + got))
done
[ "$total" -eq 31 ] || fail "(d) dry runs printed $total rows, expected 31"

# (e) a failed row does not hide the other rows (negative control with a fake cargo).
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/fail" "$tmp/pass"
cat >"$tmp/fail/cargo" <<'EOF'
#!/usr/bin/env bash
case " $* " in *" --features jieba "*) exit 1 ;; esac
exit 0
EOF
cat >"$tmp/pass/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$tmp/fail/cargo" "$tmp/pass/cargo"
cat >"$tmp/table.txt" <<'EOF'
index-text|all-features|--all-features
index-text|feature:jieba|--no-default-features --features jieba
index-text|no-default|--no-default-features
EOF

code=0
out=$(PATH="$tmp/fail:$PATH" bash "$runner" index-text "$tmp/table.txt" 2>&1) || code=$?
[ "$code" -eq 1 ] || fail "(e) failing row: exit code $code, expected 1"
grep -qxF 'ROW index-text feature:jieba FAIL' <<<"$out" || fail "(e) no FAIL line for the jieba row"
grep -qxF 'ROW index-text all-features PASS' <<<"$out" || fail "(e) all-features row did not print PASS after the failure"
grep -qxF 'ROW index-text no-default PASS' <<<"$out" || fail "(e) no-default row did not print PASS after the failure"
[ "$(tail -n 1 <<<"$out")" = "ROWS index-text: 2 passed, 1 failed" ] || fail "(e) last line is not the 2 passed, 1 failed summary"

code=0
out=$(PATH="$tmp/pass:$PATH" bash "$runner" index-text "$tmp/table.txt" 2>&1) || code=$?
[ "$code" -eq 0 ] || fail "(e) passing rows: exit code $code, expected 0"
[ "$(tail -n 1 <<<"$out")" = "ROWS index-text: 3 passed, 0 failed" ] || fail "(e) passing rows: last line is not the 3 passed, 0 failed summary"

# (f) a package with no row is refused with exit 2.
code=0
bash "$runner" no-such-crate >/dev/null 2>&1 || code=$?
[ "$code" -eq 2 ] || fail "(f) missing package: exit code $code, expected 2"

echo "feature rows ok"
