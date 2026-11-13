#!/usr/bin/env bash
# Test for the job selection (ci/select-jobs.sh) and its path map
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

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# run_pr <path>...: the selection for a pull request that changes these paths.
run_pr() {
  printf '%s\n' "$@" > "$tmp/files.txt"
  bash "$script" --event pull_request --files "$tmp/files.txt"
}

# expect <json> <group> <true|false> <label>
expect() {
  local got
  got=$(jq -r --arg g "$2" '.[$g]' <<< "$1")
  [ "$got" = "$3" ] || fail "$4: $2 is $got, expected $3 (selection: $1)"
}

# all_seven <json> <label>: every group of the map is true.
all_seven() {
  jq -e '(keys | length) == 7 and all(.[]; . == true)' <<< "$1" >/dev/null \
    || fail "$2: expected all 7 groups true (selection: $1)"
}

# (a) a change in openapi-codegen selects no e2e job.
out=$(run_pr crates/openapi-codegen/src/lib.rs)
expect "$out" e2e-service-k8s false "(a)"
expect "$out" test-openapi-codegen true "(a)"
expect "$out" lint true "(a)"
expect "$out" test-workspace true "(a)"
expect "$out" test-service-k8s false "(a)"

# (b) a change to ci.yml selects every group.
out=$(run_pr .github/workflows/ci.yml)
all_seven "$out" "(b)"

# (c) a path that the map does not name selects every group.
out=$(run_pr docs/readme.txt)
all_seven "$out" "(c)"

# (d) a change in service-k8s selects its e2e job.
out=$(run_pr crates/service-k8s/src/lib.rs)
expect "$out" e2e-service-k8s true "(d)"
expect "$out" test-service-k8s true "(d)"
expect "$out" test-openapi-codegen false "(d)"

# (e) a change in a shared crate selects every group.
out=$(run_pr crates/cli-std/src/lib.rs)
all_seven "$out" "(e)"

# (f) a change in ui-runtime selects its feature rows only.
out=$(run_pr crates/ui-runtime/src/lib.rs)
expect "$out" test-feature-rows true "(f)"
expect "$out" e2e-service-k8s false "(f)"
expect "$out" test-service-backup false "(f)"

# (g) negative control: any event that is not pull_request selects every group.
printf 'crates/openapi-codegen/src/lib.rs\n' > "$tmp/oa.txt"
out=$(bash "$script" --event push --files "$tmp/oa.txt")
all_seven "$out" "(g) push"
out=$(bash "$script" --event workflow_call --files "$tmp/oa.txt")
all_seven "$out" "(g) workflow_call"

# (h) an empty file list selects every group.
: > "$tmp/empty.txt"
out=$(bash "$script" --event pull_request --files "$tmp/empty.txt")
all_seven "$out" "(h)"

# (i) the map and the crates directory agree: every directory of crates/ has
# an entry, and every shared crate is a directory with an entry.
ls crates | sort > "$tmp/dirs.txt"
jq -r '.crates | keys[]' "$map" | sort > "$tmp/keys.txt"
cmp -s "$tmp/dirs.txt" "$tmp/keys.txt" \
  || fail "(i) the crates of $map differ from the directories of crates/"
jq -e '(.shared_crates - (.crates | keys)) | length == 0' "$map" >/dev/null \
  || fail "(i) a shared crate has no entry in $map"
while IFS= read -r name; do
  [ -d "crates/$name" ] || fail "(i) shared crate $name is not a directory of crates/"
done < <(jq -r '.shared_crates[]' "$map")

# (j) a broken map or a missing map exits 2 and prints nothing.
printf '{' > "$tmp/broken.json"
rc=0
out=$(CI_PATH_MAP="$tmp/broken.json" bash "$script" --event pull_request --files "$tmp/files.txt" 2>/dev/null) || rc=$?
[ "$rc" = "2" ] || fail "(j) broken map exited $rc, expected 2"
[ -z "$out" ] || fail "(j) broken map printed on stdout: $out"
rc=0
out=$(CI_PATH_MAP="$tmp/none.json" bash "$script" --event pull_request --files "$tmp/files.txt" 2>/dev/null) || rc=$?
[ "$rc" = "2" ] || fail "(j) missing map exited $rc, expected 2"
[ -z "$out" ] || fail "(j) missing map printed on stdout: $out"

# (k) two crates select the union of their groups.
printf 'crates/openapi-codegen/src/lib.rs\ncrates/service-backup/src/lib.rs\n' > "$tmp/two.txt"
out=$(bash "$script" --event pull_request --files "$tmp/two.txt")
expect "$out" test-openapi-codegen true "(k)"
expect "$out" test-service-backup true "(k)"
expect "$out" e2e-service-k8s false "(k)"

echo "select jobs ok"
