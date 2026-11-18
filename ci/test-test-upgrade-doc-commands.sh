#!/usr/bin/env bash
# Tests for ci/test-upgrade-doc-commands.sh (issue #358).
# Usage: bash ci/test-test-upgrade-doc-commands.sh
# Run from the repository root, like the other ci/ scripts.
set -euo pipefail

script=ci/test-upgrade-doc-commands.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# expect <name> <exit code> <output substring or -> <doc file>
# Runs the script on <doc file> and asserts the exit code and, when given,
# a substring of the combined output.
expect() {
  local name=$1 want_code=$2 want_text=$3 doc=$4 out code
  set +e
  out=$(bash "$script" "$doc" 2>&1)
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
  fi
}

# good block
cat > "$tmp/good.md" <<'EOF'
# Upgrade

<!-- tested -->
```sh
echo hi
```
EOF
expect "good block" 0 "ok: 1 blocks" "$tmp/good.md"

# bad command: the negative control
cat > "$tmp/bad.md" <<'EOF'
<!-- tested -->
```sh
false
```
EOF
expect "bad command" 1 "FAIL: block 1 failed" "$tmp/bad.md"

# a failing grep in a marked block fails too
cat > "$tmp/bad-grep.md" <<'EOF'
<!-- tested -->
```sh
grep -q nothere /dev/null
```
EOF
expect "bad grep command" 1 "FAIL: block 1 failed" "$tmp/bad-grep.md"

# an unmarked block is not run
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

# no marked block at all
cat > "$tmp/none.md" <<'EOF'
```sh
echo hi
```
EOF
expect "no marked block" 1 "no <!-- tested --> block" "$tmp/none.md"

# marker followed by text instead of a fence
cat > "$tmp/no-fence.md" <<'EOF'
<!-- tested -->
Some text.
```sh
echo hi
```
EOF
expect "marker without block" 1 "marker without an sh block" "$tmp/no-fence.md"

# marker followed by a fence of another language
cat > "$tmp/bash-fence.md" <<'EOF'
<!-- tested -->
```bash
echo hi
```
EOF
expect "marker with bash fence" 1 "marker without an sh block" "$tmp/bash-fence.md"

# missing file
expect "missing file" 1 "missing" "$tmp/does-not-exist.md"

# the real page: its three marked blocks pass
expect "real doc" 0 "ok: 3 blocks" docs/operations/upgrade.md

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "test-upgrade-doc-commands tests ok"
