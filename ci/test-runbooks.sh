#!/usr/bin/env bash
# Checks docs/operations/runbooks.md against the default alerts of service-k8s, and tests the check itself (issue #357).
# Usage: bash ci/test-runbooks.sh [root]
set -euo pipefail

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
doc="$root/docs/operations/runbooks.md"
src="$root/crates/service-k8s/src/infrastructure/manifest/monitoring.rs"
readme="$root/docs/operations/README.md"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# alert_names: the names of DEFAULT_ALERT_NAMES in the code, one per line.
alert_names() {
  sed -n '/^pub const DEFAULT_ALERT_NAMES/,/^\];/p' "$src" | grep -oE '"[A-Za-z]+"' | tr -d '"' || true
}

names=$(alert_names)
count=$(printf '%s\n' "$names" | grep -c . || true)
if [ "$count" -ne 4 ]; then
  echo "FAIL: expected 4 alert names, got $count" >&2
  exit 1
fi

# check_runbooks <file>: prints FAIL to stderr and returns 1 at the first problem.
check_runbooks() {
  local file=$1 name section label
  if [ ! -f "$file" ]; then
    echo "FAIL: missing file $file" >&2
    return 1
  fi
  for name in $names; do
    if ! grep -qxF "## $name" "$file"; then
      echo "FAIL: missing heading '## $name'" >&2
      return 1
    fi
    section=$(awk -v h="## $name" '$0 == h {on = 1; next} /^## / {on = 0} on' "$file")
    for label in '**What it means**' '**How to check**' '**How to fix**' '**When to escalate**'; do
      if ! printf '%s\n' "$section" | grep -qF "$label"; then
        echo "FAIL: section '## $name' has no $label" >&2
        return 1
      fi
    done
  done
  return 0
}

expect_pass() {
  local case=$1
  shift
  if "$@" 2>"$tmp/err"; then
    echo "ok: $case"
  else
    echo "FAIL: $case" >&2
    cat "$tmp/err" >&2
    failed=1
  fi
}

# expect_fail <case> <text that the output must contain> <file>
expect_fail() {
  local case=$1 text=$2 file=$3
  if check_runbooks "$file" 2>"$tmp/err"; then
    echo "FAIL: $case (the check passed)" >&2
    failed=1
  elif ! grep -qF "$text" "$tmp/err"; then
    echo "FAIL: $case (the output has no '$text')" >&2
    cat "$tmp/err" >&2
    failed=1
  else
    echo "ok: $case"
  fi
}

expect_pass real_runbooks_pass check_runbooks "$doc"

awk -v h='## OperatorNoLeader' '$0 == h {skip = 1; next} /^## / {skip = 0} !skip' "$doc" >"$tmp/missing.md" 2>/dev/null || true
expect_fail missing_section_fails OperatorNoLeader "$tmp/missing.md"

sed '/\*\*When to escalate\*\*/d' "$doc" >"$tmp/label.md" 2>/dev/null || true
expect_fail missing_label_fails 'When to escalate' "$tmp/label.md"

sed 's/^## ServiceDown$/## ServiceGone/' "$doc" >"$tmp/renamed.md" 2>/dev/null || true
expect_fail renamed_alert_fails ServiceDown "$tmp/renamed.md"

if grep -q '(runbooks.md)' "$readme" 2>/dev/null; then
  echo "ok: linked_from_readme"
else
  echo "FAIL: runbooks.md is not linked from README.md" >&2
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "runbooks tests ok"
