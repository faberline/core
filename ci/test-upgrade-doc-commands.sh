#!/usr/bin/env bash
# Runs the shell blocks of docs/operations/upgrade.md that follow a tested
# marker, each in its own empty directory (issue #358).
# Usage: bash ci/test-upgrade-doc-commands.sh [path/to/upgrade.md]
set -euo pipefail

file="${1:-docs/operations/upgrade.md}"
if [ ! -f "$file" ]; then
  echo "FAIL: missing $file" >&2
  exit 1
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Pass 1: write each block that follows the marker to $tmp/block-N.sh.
# States: 0 = outside, 1 = marker seen, 2 = inside a marked block.
state=0
line_no=0
marker_at=0
count=0
block=""
while IFS= read -r line || [ -n "$line" ]; do
  line_no=$((line_no + 1))
  case $state in
    0)
      if [ "$line" = '<!-- tested -->' ]; then
        state=1
        marker_at=$line_no
      fi
      ;;
    1)
      if [ "$line" != '```sh' ]; then
        echo "FAIL: marker without an sh block at line $marker_at" >&2
        exit 1
      fi
      count=$((count + 1))
      block="$tmp/block-$count.sh"
      : > "$block"
      state=2
      ;;
    2)
      if [ "$line" = '```' ]; then
        state=0
      else
        printf '%s\n' "$line" >> "$block"
      fi
      ;;
  esac
done < "$file"

if [ "$state" -eq 1 ]; then
  echo "FAIL: marker without an sh block at line $marker_at" >&2
  exit 1
fi
if [ "$state" -eq 2 ]; then
  echo "FAIL: unclosed sh block after the marker at line $marker_at" >&2
  exit 1
fi
if [ "$count" -eq 0 ]; then
  echo "FAIL: no <!-- tested --> block in $file" >&2
  exit 1
fi

# Pass 2: run every block, each in a fresh directory. Report all failures.
failed=0
for ((i = 1; i <= count; i++)); do
  dir="$tmp/run-$i"
  mkdir "$dir"
  if ! (cd "$dir" && bash -eu -o pipefail "$tmp/block-$i.sh"); then
    echo "FAIL: block $i failed" >&2
    failed=1
  fi
done

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "ok: $count blocks"
