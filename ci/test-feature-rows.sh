#!/usr/bin/env bash
# Test for the feature-row table (ci/feature-rows.txt), its runner
# (ci/run-feature-rows.sh) and the test-feature-rows job in ci.yml.
# Proves that the set of (crate, row) pairs is the same as the matrix
# that ran before one job per crate (issue #241).
# Usage: bash ci/test-feature-rows.sh
set -euo pipefail

cd "$(dirname "$0")/.."

table="ci/feature-rows.txt"
runner="ci/run-feature-rows.sh"
ci_file=".github/workflows/ci.yml"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

# Frozen (package|profile) pairs of the 31 rows of the matrix before #241.
frozen_pairs() {
  cat <<'EOF'
index-text|all-features
index-text|feature:jieba
index-text|no-default
service-auth|all-features
service-auth|feature:k8s
service-auth|no-default
service-backup|all-features
service-backup|feature:s3
service-backup|no-default
service-http|all-features
service-http|feature:otlp
service-http|no-default
service-observability|all-features
service-observability|feature:otlp
service-observability|no-default
storage-object|all-features
storage-object|feature:gcs
storage-object|feature:s3
storage-object|no-default
transport-h2c|all-features
transport-h2c|feature:server
transport-h2c|no-default
ui-runtime|all-features
ui-runtime|feature:debug
ui-runtime|feature:serde_json
ui-runtime|no-default
cli-std|all-features
cli-std|feature:k8s
cli-std|feature:online
cli-std|feature:registry
cli-std|no-default
EOF
}

# Frozen full rows (package|profile|args) of the same 31 rows.
frozen_rows() {
  cat <<'EOF'
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
}

crates="cli-std index-text service-auth service-backup service-http service-observability storage-object transport-h2c ui-runtime"

[ -f "$table" ] || fail "missing $table"
[ -f "$runner" ] || fail "missing $runner"

# Data rows: skip comment lines and empty lines.
data_rows() {
  grep -vE '^(#|$)' "$table" || true
}

# (a) exactly 31 rows, and the (package, profile) pairs equal the frozen list.
count=$(data_rows | wc -l | tr -d ' ')
[ "$count" = "31" ] || fail "table has $count data rows, expected 31"

pairs_actual=$(data_rows | cut -d'|' -f1,2 | sort)
pairs_expected=$(frozen_pairs | sort)
if [ "$pairs_actual" != "$pairs_expected" ]; then
  diff <(echo "$pairs_expected") <(echo "$pairs_actual") >&2 || true
  fail "(package, profile) pairs differ from the frozen list (a row was added or removed)"
fi

# (b) the args of each row equal the frozen args (the whole table, sorted).
rows_actual=$(data_rows | sort)
rows_expected=$(frozen_rows | sort)
if [ "$rows_actual" != "$rows_expected" ]; then
  diff <(echo "$rows_expected") <(echo "$rows_actual") >&2 || true
  fail "rows differ from the frozen table (args or a row changed)"
fi

# (c) the distinct packages are exactly the 9 crates, and the matrix of
# job test-feature-rows in ci.yml names exactly those 9 packages.
packages_table=$(data_rows | cut -d'|' -f1 | sort -u | tr '\n' ' ' | sed 's/ $//')
packages_expected=$(echo "$crates" | tr ' ' '\n' | sort | tr '\n' ' ' | sed 's/ $//')
[ "$packages_table" = "$packages_expected" ] || fail "table packages are '$packages_table', expected '$packages_expected'"

packages_matrix=$(awk '/^  test-feature-rows:/{f=1; next} /^  [a-z0-9-]+:$/{f=0} f' "$ci_file" \
  | { grep -oE '^ *- \{ package: [a-z0-9-]+ \}' || true; } \
  | sed -E 's/.*package: ([a-z0-9-]+) \}/\1/' \
  | sort | tr '\n' ' ' | sed 's/ $//')
[ "$packages_matrix" = "$packages_expected" ] || fail "matrix of test-feature-rows is '$packages_matrix', expected '$packages_expected'"

# (d) a dry run per package prints one DRY line per row; the sum is 31.
dry_total=0
for pkg in $crates; do
  expected_n=$(data_rows | cut -d'|' -f1 | grep -cx "$pkg" || true)
  got_n=$(FEATURE_ROWS_DRY_RUN=1 bash "$runner" "$pkg" | grep -c "^DRY $pkg " || true)
  [ "$got_n" = "$expected_n" ] || fail "dry run of $pkg printed $got_n DRY lines, expected $expected_n"
  dry_total=$((dry_total + got_n))
done
[ "$dry_total" = "31" ] || fail "dry runs printed $dry_total rows, expected 31"

# (e) failure does not hide rows (negative control with a fake cargo).
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"
cat > "$tmp/bin/cargo" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"--features jieba"*) exit 1 ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$tmp/bin/cargo"
cat > "$tmp/table.txt" <<'EOF'
index-text|all-features|--all-features
index-text|feature:jieba|--no-default-features --features jieba
index-text|no-default|--no-default-features
EOF

rc=0
out=$(PATH="$tmp/bin:$PATH" bash "$runner" index-text "$tmp/table.txt" 2>&1) || rc=$?
[ "$rc" = "1" ] || fail "index-text with one failing row exited $rc, expected 1"
echo "$out" | grep -qx "ROW index-text feature:jieba FAIL" || fail "missing 'ROW index-text feature:jieba FAIL'"
echo "$out" | grep -qx "ROW index-text all-features PASS" || fail "missing 'ROW index-text all-features PASS'"
echo "$out" | grep -qx "ROW index-text no-default PASS" || fail "missing 'ROW index-text no-default PASS'"
echo "$out" | tail -n 1 | grep -q "2 passed, 1 failed" || fail "last line does not say '2 passed, 1 failed'"

cat > "$tmp/bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$tmp/bin/cargo"
rc=0
out=$(PATH="$tmp/bin:$PATH" bash "$runner" index-text "$tmp/table.txt" 2>&1) || rc=$?
[ "$rc" = "0" ] || fail "index-text with all rows passing exited $rc, expected 0"
echo "$out" | tail -n 1 | grep -q "3 passed, 0 failed" || fail "last line does not say '3 passed, 0 failed'"

# (f) a missing package exits 2.
rc=0
bash "$runner" no-such-crate >/dev/null 2>&1 || rc=$?
[ "$rc" = "2" ] || fail "no-such-crate exited $rc, expected 2"

echo "feature rows ok"
