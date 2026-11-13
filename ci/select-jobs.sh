#!/usr/bin/env bash
# Select the CI groups that a change needs (issue #318).
# Prints one line of JSON on stdout: every group of the path map is a key,
# with true when the change needs it. Messages go to stderr only.
#
# A push, and any event that is not pull_request, selects every group. On a
# pull request, a file that matches run_all_paths, a file of a shared crate,
# a file that no rule names, or an empty list also selects every group.
# Otherwise the groups are the union of the groups that the map gives each
# changed crate.
#
# Usage: bash ci/select-jobs.sh [--event <name>] [--files <file>]
#   --event  defaults to the env GITHUB_EVENT_NAME
#   --files  one changed path per line; without it, a pull_request reads
#            git diff --name-only origin/main...HEAD
# Env: CI_PATH_MAP  the path map (default .github/ci-path-map.json at the repo root)
set -euo pipefail

usage() {
  echo "usage: bash ci/select-jobs.sh [--event <name>] [--files <file>]" >&2
}

event="${GITHUB_EVENT_NAME:-}"
files_arg=""
while [ $# -gt 0 ]; do
  case "$1" in
    --event)
      [ $# -ge 2 ] || { usage; exit 2; }
      event="$2"
      shift 2
      ;;
    --files)
      [ $# -ge 2 ] || { usage; exit 2; }
      files_arg="$2"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

here="$(cd "$(dirname "$0")" && pwd)"
map="${CI_PATH_MAP:-$here/../.github/ci-path-map.json}"

# Check the map first: a broken map must never give a partial answer.
if [ ! -f "$map" ] || ! jq -e . "$map" >/dev/null 2>&1; then
  echo "invalid or missing path map: $map" >&2
  exit 2
fi

# all_true: the JSON line with every group true.
all_true() {
  jq -c '[.all_groups[] | {key: ., value: true}] | from_entries' "$map"
}

if [ "$event" != "pull_request" ]; then
  all_true
  exit 0
fi

if [ -n "$files_arg" ]; then
  if [ ! -r "$files_arg" ]; then
    echo "missing files list: $files_arg" >&2
    exit 2
  fi
  files=$(cat "$files_arg")
else
  files=$(git diff --name-only origin/main...HEAD) || {
    echo "git diff origin/main...HEAD failed: cannot list the changed files" >&2
    exit 2
  }
fi

changed=()
while IFS= read -r line; do
  if [ -n "$line" ]; then
    changed+=("$line")
  fi
done <<< "$files"

if [ "${#changed[@]}" -eq 0 ]; then
  all_true
  exit 0
fi

run_all=$(jq -r '.run_all_paths[]' "$map")
shared=$(jq -r '.shared_crates[]' "$map")

selected=""
for file in "${changed[@]}"; do
  # 1. A file that always runs every group.
  while IFS= read -r glob; do
    [ -n "$glob" ] || continue
    pattern="${glob//\*\*/*}"
    # shellcheck disable=SC2254
    case "$file" in
      $pattern)
        all_true
        exit 0
        ;;
    esac
  done <<< "$run_all"

  # 2. A file outside crates/ is named by no rule.
  case "$file" in
    crates/*) ;;
    *)
      all_true
      exit 0
      ;;
  esac

  # 3. The crate name is the second segment. A file directly in crates/,
  # a shared crate, or a crate that the map does not know selects every group.
  rest="${file#crates/}"
  case "$rest" in
    */*) ;;
    *)
      all_true
      exit 0
      ;;
  esac
  name="${rest%%/*}"
  if grep -qxF -- "$name" <<< "$shared"; then
    all_true
    exit 0
  fi
  if ! jq -e --arg c "$name" '.crates | has($c)' "$map" >/dev/null; then
    all_true
    exit 0
  fi

  # 4. The groups that the map gives this leaf crate.
  selected+=$'\n'"$(jq -r --arg c "$name" '.crates[$c].groups[]' "$map")"
done

jq -c --arg sel "$selected" '
  ($sel | split("\n")) as $s
  | [.all_groups[] | {key: ., value: (. as $g | any($s[]; . == $g))}]
  | from_entries' "$map"
