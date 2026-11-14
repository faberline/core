#!/usr/bin/env bash
# Tests for ci/check-threat-model.sh (issue #354).
# Usage: bash ci/test-check-threat-model.sh
set -euo pipefail

cd "$(dirname "$0")/.."
script=ci/check-threat-model.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=0

# make_root <name> <threat-model.md content | MISSING> <README.md content>
# Creates a fixture root under $tmp with crates/alpha/lib.rs and prints its path.
make_root() {
  local dir="$tmp/$1" doc=$2 readme=$3
  mkdir -p "$dir/docs/operations" "$dir/crates/alpha"
  printf '%s\n' '// alpha' > "$dir/crates/alpha/lib.rs"
  if [ "$doc" != MISSING ]; then
    printf '%s\n' "$doc" > "$dir/docs/operations/threat-model.md"
  fi
  printf '%s\n' "$readme" > "$dir/docs/operations/README.md"
  echo "$dir"
}

# expect <name> <exit code> <output substring or -> <root>
# Runs the check on <root> and asserts the exit code and, when given,
# a substring of the combined output.
expect() {
  local name=$1 want_code=$2 want_text=$3 root=$4 out code
  set +e
  out=$(bash "$script" "$root" 2>&1)
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

# full_doc [omit-heading] [extra line]: a threat model with all six sections.
full_doc() {
  local omit=${1:-} extra=${2:-}
  printf '%s\n' '# Threat model' '' 'The independent review is a person'"'"'s task.' '' \
    '## Auth' '' 'Control: `crates/alpha/lib.rs`.' '' \
    '## TLS' '' '## Audit' '' '## Secrets' '' \
    '## Supply chain of the git-tag dependency model' '' \
    '## Not applicable to core' '' 'Images do not apply.' '' "$extra"
  return 0
}

# Remove one heading from full_doc output.
without() {
  full_doc "" "" | grep -vxF -- "$1"
}

# 1. all_ok: six sections, the review, one cited path that exists, README link.
root=$(make_root all_ok "$(full_doc)" 'See [threat-model.md](threat-model.md).')
expect all_ok 0 'threat model ok: 6 sections, 1 cited paths' "$root"

# 2. missing_document_fails (negative): no threat-model.md.
root=$(make_root missing_document_fails MISSING 'See [threat-model.md](threat-model.md).')
expect missing_document_fails 1 missing "$root"

# 3. missing_section_fails (negative): the Secrets heading is gone.
root=$(make_root missing_section_fails "$(without '## Secrets')" 'See [threat-model.md](threat-model.md).')
expect missing_section_fails 1 "section '## Secrets' is missing" "$root"

# 4. missing_not_applicable_fails (negative): the last section is gone.
root=$(make_root missing_not_applicable_fails "$(without '## Not applicable to core')" 'See [threat-model.md](threat-model.md).')
expect missing_not_applicable_fails 1 "Not applicable to core" "$root"

# 5. missing_path_fails (negative): a cited path that does not exist.
root=$(make_root missing_path_fails "$(full_doc "" 'Control: `crates/ghost/src/lib.rs`.')" 'See [threat-model.md](threat-model.md).')
expect missing_path_fails 1 "cited path does not exist: crates/ghost/src/lib.rs" "$root"

# 6. no_readme_link_fails (negative): the README does not link the document.
root=$(make_root no_readme_link_fails "$(full_doc)" 'No link here.')
expect no_readme_link_fails 1 "does not link threat-model.md" "$root"

# 7. real_repo_ok: the real repository passes.
expect real_repo_ok 0 "threat model ok: 6 sections" "$(pwd)"

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "check-threat-model tests ok"
