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

# The first step of lint is the shape check itself.
grep -q 'name: "Check ci.yml shape"' "$file" || grep -q 'name: Check ci.yml shape' "$file" \
  || report "lint has no step named Check ci.yml shape"

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
