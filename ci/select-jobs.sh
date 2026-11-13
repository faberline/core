#!/usr/bin/env bash
# Select the CI groups that a change needs (issue #318).
# Reads the changed files and prints one line of JSON: each of the 7 groups
# of the path map is a key, with true when the change needs it.
# Usage: bash ci/select-jobs.sh [--event <name>] [--files <file>]
#   --event  defaults to $GITHUB_EVENT_NAME
#   --files  one changed path per line. Without it, a pull_request takes the
#            files from `git diff --name-only origin/main...HEAD`.
# Env: CI_PATH_MAP replaces the map (default .github/ci-path-map.json).
# Rules: any event that is not pull_request selects every group. So does a
# pull_request with no changed file, a file under run_all_paths, a file
# under a shared crate, and a file that no rule names. A missing or broken
# map exits 2 and prints nothing on stdout.
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

if [ ! -f "$map" ] || ! jq -e . "$map" >/dev/null 2>&1; then
  echo "invalid or missing path map: $map" >&2
  exit 2
fi

# Every group true, as one JSON line.
all_true() {
  jq -c '[.all_groups[] | {key: ., value: true}] | from_entries' "$map"
}

if [ "$event" != "pull_request" ]; then
  all_true
  exit 0
fi

if [ -n "$files_arg" ]; then
  if [ ! -f "$files_arg" ]; then
    echo "missing files list: $files_arg" >&2
    exit 2
  fi
  files=$(cat "$files_arg")
else
  files=$(git diff --name-only origin/main...HEAD) || {
    echo "git diff failed: cannot list the changed files" >&2
    exit 2
  }
fi
files=$(printf '%s\n' "$files" | grep -v '^$' || true)

if [ -z "$files" ]; then
  all_true
  exit 0
fi

# Groups selected so far, space separated, with a space before and after each.
selected=" "
while IFS= read -r file; do
  # A file that matches a run_all_paths glob runs everything. In a case
  # pattern `*` also matches `/`, so `.github/**` becomes `.github/*`.
  while IFS= read -r glob; do
    pat="${glob//\*\*/*}"
    case "$file" in
      $pat)
        all_true
        exit 0
        ;;
    esac
  done < <(jq -r '.run_all_paths[]' "$map")

  # Only crates/<name>/... has a rule; any other path is unnamed.
  case "$file" in
    crates/*) ;;
    *)
      all_true
      exit 0
      ;;
  esac
  rest="${file#crates/}"
  case "$rest" in
    */*) name="${rest%%/*}" ;;
    *)
      all_true
      exit 0
      ;;
  esac

  # A shared crate, or a crate that the map does not know, runs everything.
  if jq -e --arg n "$name" '.shared_crates | index($n) != null' "$map" >/dev/null; then
    all_true
    exit 0
  fi
  if ! jq -e --arg n "$name" '.crates | has($n)' "$map" >/dev/null; then
    all_true
    exit 0
  fi

  groups=$(jq -r --arg n "$name" '.crates[$n].groups | join(" ")' "$map")
  selected="$selected$groups "
done <<< "$files"

jq -c --arg sel "$selected" \
  '[.all_groups[] | . as $g | {key: $g, value: ($sel | contains(" " + $g + " "))}] | from_entries' "$map"
