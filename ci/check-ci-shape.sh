#!/usr/bin/env bash
# Shape check for .github/workflows/ci.yml (issue #140, aw docs/standards/ci.md).
# Usage: bash ci/check-ci-shape.sh [path/to/ci.yml]
set -euo pipefail

file="${1:-.github/workflows/ci.yml}"
fail=0

report() {
  echo "FAIL: $1" >&2
  fail=1
}

if [ ! -f "$file" ]; then
  echo "FAIL: missing $file" >&2
  exit 1
fi

# Workflow name must be exactly `ci`.
grep -qE '^name: ci$' "$file" || report "workflow name is not ci"

# Job ids are the two-space-indented keys under `jobs:`.
job_ids=$(awk '/^jobs:/{injobs=1; next} injobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/{gsub(/[ :]/,""); print}' "$file")
grep -qx 'lint' <<<"$job_ids" || report "no job id lint"
grep -qx 'gate' <<<"$job_ids" || report "no job id gate"
grep -q '^test-' <<<"$job_ids" || report "no job id starting with test-"

# Text of one job: from its `  <id>:` line to the next job id.
job_block() {
  awk -v id="$1" '
    $0 == "  " id ":" {on=1; next}
    on && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {on=0}
    on {print}' "$file"
}

# Display names: lint, test-* and e2e-* use `<stage>: <what>`; gate is `gate`.
while IFS= read -r id; do
  [ -n "$id" ] || continue
  name=$(job_block "$id" | sed -n 's/^    name: *//p' | head -1 | tr -d '"')
  case "$id" in
    gate) [ "$name" = "gate" ] || report "job gate is not named gate (name: $name)" ;;
    lint) [[ "$name" == "lint: "* ]] || report "job lint has no lint: display name" ;;
    test-*) [[ "$name" == "test: "* ]] || report "job $id has no test: display name" ;;
    e2e-*) [[ "$name" == "e2e: "* ]] || report "job $id has no e2e: display name" ;;
  esac
done <<<"$job_ids"

# Every test-* job needs lint; every e2e-* job needs a test-* job.
while IFS= read -r id; do
  case "$id" in
    test-*)
      job_block "$id" | grep -E '^    needs: .*\blint\b' >/dev/null || report "$id does not need lint" ;;
    e2e-*)
      job_block "$id" | grep -E '^    needs: .*\btest-' >/dev/null || report "$id does not need a test-* job" ;;
  esac
done <<<"$job_ids"

# gate runs always and needs every other job.
gate_block=$(job_block gate)
grep -qE "^    if: .*always\(\)" <<<"$gate_block" || report "gate does not have if: always()"
gate_needs=$(sed -n 's/^    needs: *//p' <<<"$gate_block")
while IFS= read -r id; do
  [ -n "$id" ] && [ "$id" != "gate" ] || continue
  grep -qE "(^|[][, ])${id}([],]|\$)" <<<"$gate_needs" || report "gate does not need $id"
done <<<"$job_ids"

# Cancel only pull request runs.
grep -qxF "  cancel-in-progress: \${{ github.event_name == 'pull_request' }}" "$file" \
  || report "cancel-in-progress is not limited to pull_request"

# Lint runs exactly two commands: fmt, then clippy.
lint_cargo=$(job_block lint | grep -E '(^|[[:space:]])cargo[[:space:]]' || true)
if [ "$(grep -c . <<<"$lint_cargo")" -ne 2 ] \
  || ! sed -n 1p <<<"$lint_cargo" | grep -qF 'cargo fmt --all -- --check' \
  || ! sed -n 2p <<<"$lint_cargo" | grep -qF 'cargo clippy --workspace --all-targets --locked -- -D warnings'; then
  report "lint does not run exactly cargo fmt and cargo clippy"
fi

# The first step of lint is the shape check itself.
job_block lint | grep -q 'name: Check ci.yml shape' || report "lint has no step named Check ci.yml shape"

# Every uses: line is pinned to a full 40-hex commit SHA.
while IFS= read -r line; do
  if ! grep -qE 'uses: [^ ]+@[0-9a-f]{40}' <<<"$line"; then
    report "action not pinned to a full SHA: $line"
  fi
done < <(grep -E 'uses:' "$file" || true)

# Every cargo command except cargo fmt carries --locked.
while IFS= read -r line; do
  if grep -qE 'cargo fmt' <<<"$line"; then
    continue
  fi
  if ! grep -qE -- '--locked' <<<"$line"; then
    report "cargo command without --locked: $line"
  fi
done < <(grep -E '(^|[[:space:]])cargo[[:space:]]' "$file" || true)

# No step may read the issue, the pull request body, labels or comments.
if grep -nE 'issue-ci|issue-contract|github\.event\.issue|pull_request\.body|labels' "$file"; then
  report "ci.yml reads issue, PR body or labels (matches above)"
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "ci.yml shape ok: $file"
