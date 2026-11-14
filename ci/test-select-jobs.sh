#!/usr/bin/env bash
# Test for the job selection (ci/select-jobs.sh) and its reviewed path map
# (.github/ci-path-map.json), run by the lint job of ci.yml (issue #318).
# Usage: bash ci/test-select-jobs.sh
set -euo pipefail

cd "$(dirname "$0")/.."

script="ci/select-jobs.sh"
map=".github/ci-path-map.json"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

[ -f "$script" ] || fail "missing $script"
[ -f "$map" ] || fail "missing $map"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# run_pr <path>...: the selection for a pull request that changes these files.
run_pr() {
  printf '%s\n' "$@" > "$tmp/files.txt"
  bash "$script" --event pull_request --files "$tmp/files.txt"
}

# check <json> <group> <true|false> <label>
check() {
  local json="$1" group="$2" want="$3" label="$4" got
  got=$(echo "$json" | jq -r --arg g "$group" '.[$g]')
  [ "$got" = "$want" ] || fail "$label: $group is $got, expected $want"
}

# all_true <json> <label>: all 8 groups are true.
all_true() {
  echo "$1" | jq -e '(keys | length) == 8 and all(.[]; . == true)' >/dev/null \
    || fail "$2: expected all 8 groups true, got $1"
}

# (a) a change to openapi-codegen selects no kind e2e job.
out=$(run_pr crates/openapi-codegen/src/lib.rs)
check "$out" e2e-service-k8s false "(a)"
check "$out" test-openapi-codegen true "(a)"
check "$out" lint true "(a)"
check "$out" test-workspace true "(a)"
check "$out" test-service-k8s false "(a)"

# (b) a change to the workflow selects all groups.
out=$(run_pr .github/workflows/ci.yml)
all_true "$out" "(b)"

# (c) a path that the map does not name selects all groups.
out=$(run_pr docs/readme.txt)
all_true "$out" "(c)"

# (d) a change to service-k8s selects its e2e and its test job.
out=$(run_pr crates/service-k8s/src/lib.rs)
check "$out" e2e-service-k8s true "(d)"
check "$out" test-service-k8s true "(d)"
check "$out" test-openapi-codegen false "(d)"

# (e) a change to a shared crate selects all groups.
out=$(run_pr crates/cli-std/src/lib.rs)
all_true "$out" "(e)"

# (f) a change to ui-runtime selects the feature rows, not the kind e2e.
out=$(run_pr crates/ui-runtime/src/lib.rs)
check "$out" test-feature-rows true "(f)"
check "$out" e2e-service-k8s false "(f)"
check "$out" test-service-backup false "(f)"

# (g) negative control: any event that is not pull_request selects all groups,
# even for a file that alone would select only a few.
printf '%s\n' crates/openapi-codegen/src/lib.rs > "$tmp/one.txt"
out=$(bash "$script" --event push --files "$tmp/one.txt")
all_true "$out" "(g) push"
out=$(bash "$script" --event workflow_call --files "$tmp/one.txt")
all_true "$out" "(g) workflow_call"

# (h) an empty file list selects all groups.
: > "$tmp/empty.txt"
out=$(bash "$script" --event pull_request --files "$tmp/empty.txt")
all_true "$out" "(h)"

# (i) the map names every crate directory, and every shared crate is one.
dirs=$(find crates -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | LC_ALL=C sort)
keys=$(jq -r '.crates | keys[]' "$map" | LC_ALL=C sort)
[ "$dirs" = "$keys" ] || {
  diff <(echo "$dirs") <(echo "$keys") >&2 || true
  fail "(i) crates/ directories and the keys of .crates differ"
}
while IFS= read -r c; do
  [ -n "$c" ] || continue
  [ -d "crates/$c" ] || fail "(i) shared crate $c is not a directory of crates/"
  jq -e --arg c "$c" '.crates | has($c)' "$map" >/dev/null \
    || fail "(i) shared crate $c is not a key of .crates"
done < <(jq -r '.shared_crates[]' "$map")

# (j) a broken map or a missing map exits 2 and prints nothing on stdout.
printf '{' > "$tmp/broken.json"
rc=0
out=$(CI_PATH_MAP="$tmp/broken.json" bash "$script" --event pull_request --files "$tmp/one.txt" 2>/dev/null) || rc=$?
[ "$rc" = "2" ] || fail "(j) broken map exited $rc, expected 2"
[ -z "$out" ] || fail "(j) broken map printed '$out' on stdout"

rc=0
out=$(CI_PATH_MAP="$tmp/none.json" bash "$script" --event pull_request --files "$tmp/one.txt" 2>/dev/null) || rc=$?
[ "$rc" = "2" ] || fail "(j) missing map exited $rc, expected 2"
[ -z "$out" ] || fail "(j) missing map printed '$out' on stdout"

# (k) two crates select the union of their groups.
printf '%s\n' crates/openapi-codegen/src/lib.rs crates/service-backup/src/lib.rs > "$tmp/two.txt"
out=$(bash "$script" --event pull_request --files "$tmp/two.txt")
check "$out" test-openapi-codegen true "(k)"
check "$out" test-service-backup true "(k)"
check "$out" e2e-service-k8s false "(k)"

# (l) a crate that is in the semver check selects test-semver; e2e-support
#     is not published, so it is skipped by the check and selects nothing.
out=$(run_pr crates/service-k8s/src/lib.rs)
check "$out" test-semver true "(l)"
out=$(run_pr crates/e2e-support/src/lib.rs)
check "$out" test-semver false "(l)"

echo "select jobs ok"
