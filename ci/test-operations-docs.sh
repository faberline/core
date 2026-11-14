#!/usr/bin/env bash
# Tests for the operations documents of issue 356: configuration-reference.md and
# limits.md must exist, be linked from docs/operations/README.md, and cite only
# source paths that exist.
# Usage: bash ci/test-operations-docs.sh (from the repository root)
set -euo pipefail

cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0
docs=(configuration-reference.md limits.md)

# check_docs <root>: print a FAIL line per problem; return 1 when there is one.
check_docs() {
  local root=$1 name file path bad=0
  for name in "${docs[@]}"; do
    file="$root/docs/operations/$name"
    if [ ! -f "$file" ]; then
      echo "FAIL: $name is missing"
      bad=1
      continue
    fi
    if ! grep -q "($name)" "$root/docs/operations/README.md"; then
      echo "FAIL: $name is not linked from README.md"
      bad=1
    fi
    while IFS= read -r path; do
      path=${path#\`}
      path=${path%\`}
      path=${path%%:[0-9]*}
      if [ ! -e "$root/$path" ]; then
        echo "FAIL: $name cites a missing path $path"
        bad=1
      fi
    done < <(grep -oE '`crates/[^`]+`' "$file" || true)
  done
  return "$bad"
}

# mkroot <name>: a root with copies of the real operations documents and a link
# to the real crates directory.
mkroot() {
  local r="$tmp/$1"
  mkdir -p "$r/docs/operations"
  cp docs/operations/configuration-reference.md docs/operations/limits.md \
    docs/operations/README.md "$r/docs/operations/"
  ln -s "$PWD/crates" "$r/crates"
  echo "$r"
}

# real_docs_pass (positive)
out=$(check_docs .) && code=0 || code=$?
if [ "$code" -eq 0 ]; then
  echo "ok: real_docs_pass"
else
  echo "FAIL: real_docs_pass: $out"
  failed=1
fi

# missing_document_fails (negative)
r=$(mkroot missing)
rm "$r/docs/operations/limits.md"
out=$(check_docs "$r") && code=0 || code=$?
if [ "$code" -ne 0 ] && grep -q 'limits.md is missing' <<<"$out"; then
  echo "ok: missing_document_fails"
else
  echo "FAIL: missing_document_fails (exit $code): $out"
  failed=1
fi

# unlinked_document_fails (negative)
r=$(mkroot unlinked)
sed -i '/(limits.md)/d' "$r/docs/operations/README.md"
out=$(check_docs "$r") && code=0 || code=$?
if [ "$code" -ne 0 ] && grep -q 'is not linked' <<<"$out"; then
  echo "ok: unlinked_document_fails"
else
  echo "FAIL: unlinked_document_fails (exit $code): $out"
  failed=1
fi

# missing_cited_path_fails (negative)
r=$(mkroot cited)
sed -i 's#crates/service-http/src/infrastructure/config.rs#crates/service-http/src/infrastructure/gone.rs#' \
  "$r/docs/operations/configuration-reference.md"
out=$(check_docs "$r") && code=0 || code=$?
if [ "$code" -ne 0 ] && grep -q 'cites a missing path' <<<"$out"; then
  echo "ok: missing_cited_path_fails"
else
  echo "FAIL: missing_cited_path_fails (exit $code): $out"
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "operations-docs tests ok"
