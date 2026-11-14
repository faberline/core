#!/usr/bin/env bash
# Checks that every #[ignore] under crates/ has a reason string, so that no
# test is skipped without a stated cause (issue #350).
# Usage: bash ci/check-ignore-reasons.sh [root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
crates="$root/crates"

if [ ! -d "$crates" ]; then
  echo "FAIL: missing $crates" >&2
  exit 1
fi

fail=0
# An ignore with no reason: `#[ignore]` or `#[ignore = ""]`.
while IFS= read -r hit; do
  [ -n "$hit" ] || continue
  file=${hit%%:*}
  rest=${hit#*:}
  line=${rest%%:*}
  rel=${file#"$root/"}
  echo "FAIL: $rel:$line: #[ignore] has no reason" >&2
  fail=1
done < <(grep -rnE --include='*.rs' '#\[ignore[[:space:]]*(\]|=[[:space:]]*""[[:space:]]*\])' "$crates" || true)

if [ "$fail" -ne 0 ]; then
  exit 1
fi

count=$(grep -rhE --include='*.rs' '#\[ignore' "$crates" | wc -l || true)
echo "ignore reasons ok: $count ignored tests"
