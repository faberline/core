#!/usr/bin/env bash
# Tests for ci/test-upgrade-doc-commands.sh (issue #358).
# Usage: bash ci/test-test-upgrade-doc-commands.sh
set -euo pipefail

runner=ci/test-upgrade-doc-commands.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# expect <name> <exit code> <output substring or -> <file>
# Runs the runner on <file> and asserts the exit code and, when given,
# a substring of the combined output.
expect() {
  local name=$1 want_code=$2 want_text=$3 file=$4 out code
  set +e
  out=$(bash "$runner" "$file" 2>&1)
  code=$?
  set -e
  if [ "$code" -ne "$want_code" ]; then
    echo "FAIL: $name (exit $code, want $want_code): $out"
    failed=1
    return
  fi
  if [ "$want_text" != "-" ] && ! grep -qF -- "$want_text" <<<"$out"; then
    echo "FAIL: $name (output lacks '$want_text'): $out"
    failed=1
    return
  fi
  echo "ok: $name"
}

# good block: one marked block that succeeds.
cat > "$tmp/good.md" <<'EOF'
# Page

<!-- tested -->
```sh
echo hi
```
EOF
expect "good block" 0 "ok: 1 blocks" "$tmp/good.md"

# bad command (the negative control): a failing marked block must fail the run.
cat > "$tmp/bad.md" <<'EOF'
<!-- tested -->
```sh
false
```
EOF
expect "bad command" 1 "FAIL: block 1 failed" "$tmp/bad.md"

cat > "$tmp/bad-grep.md" <<'EOF'
<!-- tested -->
```sh
grep -q nothere /dev/null
```
EOF
expect "bad grep command" 1 "FAIL: block 1 failed" "$tmp/bad-grep.md"

# unmarked block ignored: a failing block without the marker does not run.
cat > "$tmp/unmarked.md" <<'EOF'
<!-- tested -->
```sh
echo hi
```

```sh
false
```
EOF
expect "unmarked block ignored" 0 "ok: 1 blocks" "$tmp/unmarked.md"

# no marked block: only unmarked blocks.
cat > "$tmp/none.md" <<'EOF'
```sh
echo hi
```
EOF
expect "no marked block" 1 "no <!-- tested --> block" "$tmp/none.md"

# marker without block: the marker is followed by text, not a sh fence.
cat > "$tmp/marker-text.md" <<'EOF'
<!-- tested -->
some text
EOF
expect "marker without block" 1 "marker without an sh block" "$tmp/marker-text.md"

# missing file.
expect "missing file" 1 "missing" "$tmp/does-not-exist.md"

# real doc: the blocks of docs/operations/upgrade.md all pass.
expect "real doc" 0 "ok: 3 blocks" docs/operations/upgrade.md

if [ "$failed" -eq 0 ]; then
  echo "ok"
fi
exit "$failed"
