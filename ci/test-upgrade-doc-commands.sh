#!/usr/bin/env bash
# Runs every block marked `<!-- tested -->` in an upgrade page (issue #358).
# Each block runs in its own fresh directory under a temporary root.
# Usage: bash ci/test-upgrade-doc-commands.sh [path/to/upgrade.md]
set -euo pipefail

file="${1:-docs/operations/upgrade.md}"

if [ ! -f "$file" ]; then
  echo "FAIL: missing $file" >&2
  exit 1
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Extract each marked block to $tmp/block-N.sh. The marker must be followed
# by exactly "```sh"; the block ends at the next line that is exactly "```".
count=$(awk -v dir="$tmp" '
  BEGIN { n = 0; state = 0 }
  state == 1 {
    if ($0 != "```sh") {
      print "FAIL: marker without an sh block at line " mline > "/dev/stderr"
      state = 3
      exit 1
    }
    n++
    out = dir "/block-" n ".sh"
    printf "" > out
    state = 2
    next
  }
  state == 2 {
    if ($0 == "```") { state = 0; close(out); next }
    print $0 > out
    next
  }
  $0 == "<!-- tested -->" { state = 1; mline = NR; next }
  END {
    if (state == 3) exit 1
    if (state == 2) {
      print "FAIL: unclosed sh block at end of " FILENAME > "/dev/stderr"
      exit 1
    }
    if (state == 1) {
      print "FAIL: marker without an sh block at line " mline > "/dev/stderr"
      exit 1
    }
    print n
  }
' "$file")

if [ "$count" -eq 0 ]; then
  echo "FAIL: no <!-- tested --> block in $file" >&2
  exit 1
fi

failed=0
for i in $(seq 1 "$count"); do
  dir="$tmp/run-$i"
  mkdir -p "$dir"
  if ! (cd "$dir" && bash -euo pipefail "$tmp/block-$i.sh"); then
    echo "FAIL: block $i failed"
    failed=1
  fi
done

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "ok: $count blocks"
