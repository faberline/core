#!/usr/bin/env bash
# Checks docs/operations/failure-drills.md and tests the check itself (issue #361).
# Usage: bash ci/test-failure-drills-doc.sh [root]
set -euo pipefail

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
doc="$root/docs/operations/failure-drills.md"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

headings=(
  "## Leader loss"
  "## Node loss"
  "## Network partition"
  "## Disk full"
  "## Backup and restore"
  "## Rolling upgrade"
  "## 24 hour soak"
)

# check_doc <file>: prints FAIL to stderr and returns 1 at the first problem.
check_doc() {
  local file=$1 h label section value
  if [ ! -f "$file" ]; then
    echo "FAIL: missing file $file" >&2
    return 1
  fi
  for h in "${headings[@]}"; do
    if ! grep -qxF "$h" "$file"; then
      echo "FAIL: missing heading '$h'" >&2
      return 1
    fi
  done
  if ! grep -qF 'only after a person approves the cost figure' "$file"; then
    echo "FAIL: the approval rule is missing" >&2
    return 1
  fi
  for h in "${headings[@]}"; do
    section=$(awk -v h="$h" '$0 == h {on = 1; next} /^## / {on = 0} on' "$file")
    for label in '- Runs on:' '- Cost estimate:' '- Commands:' '- Expected result:'; do
      if ! printf '%s\n' "$section" | grep -q "^$label"; then
        echo "FAIL: section '$h' has no line starting with '$label'" >&2
        return 1
      fi
    done
    value=$(printf '%s\n' "$section" | awk '/^- Cost estimate: / {sub(/^- Cost estimate: /, ""); print; exit}')
    if ! printf '%s\n' "$value" | grep -qE '^(no estimate|[0-9])'; then
      echo "FAIL: section '$h': cost estimate must be a figure or \"no estimate\"" >&2
      return 1
    fi
  done
  return 0
}

if check_doc "$doc"; then
  echo "ok: real document"
else
  echo "FAIL: real document" >&2
  failed=1
fi

# expect_failure <name> <file>: check_doc must reject the file.
expect_failure() {
  if check_doc "$2" 2>/dev/null; then
    echo "FAIL: $1 (the check passed, it must fail)" >&2
    failed=1
  else
    echo "ok: $1"
  fi
}

expect_failure missing_file_fails "$tmp/does-not-exist.md"

grep -vxF '## Disk full' "$doc" > "$tmp/removed-heading.md" || true
expect_failure removed_heading_fails "$tmp/removed-heading.md"

grep -v '^- Cost estimate:' "$doc" > "$tmp/removed-cost.md" || true
expect_failure removed_cost_estimate_fails "$tmp/removed-cost.md"

sed 's/^- Cost estimate: no estimate/- Cost estimate: cheap/' "$doc" > "$tmp/bad-cost.md"
expect_failure bad_cost_estimate_fails "$tmp/bad-cost.md"

sed 's/only after a person approves the cost figure/x/' "$doc" > "$tmp/no-approval.md"
expect_failure removed_approval_text_fails "$tmp/no-approval.md"

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "failure drills doc tests ok"
