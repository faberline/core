#!/usr/bin/env bash
# Writes the objects that a release tag renders for the upgrade-rollback harness
# (#372) as a JSON array. The example render_dump runs in a git worktree of the
# old tag, so the old render comes from that tag's own code.
# Usage: bash ci/upgrade-rollback-old-render.sh <out-file> [<tag>]
#   <tag> defaults to the newest final v0.* tag (no `-` in it) of the clone.
# Env: UPGRADE_ROLLBACK_TARGET_DIR reuses a cargo build dir between runs.
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "usage: bash ci/upgrade-rollback-old-render.sh <out-file> [<tag>]" >&2
  exit 2
fi

root=$(cd "$(dirname "$0")/.." && pwd)
out=$(realpath -m "$1")
tag="${2:-$(git -C "$root" tag -l 'v0.*' | grep -v -- '-' | sort -V | tail -1)}"

if [ -z "$tag" ]; then
  echo "FAIL: no v0.* release tag in the clone (run: git fetch --tags)" >&2
  exit 1
fi

tmp=$(mktemp -d)
trap 'git -C "$root" worktree remove --force "$tmp/old" >/dev/null 2>&1 || true; rm -rf "$tmp"' EXIT

if ! git -C "$root" worktree add --detach "$tmp/old" "$tag" >/dev/null 2>&1; then
  echo "FAIL: tag $tag not found in the clone" >&2
  exit 1
fi

mkdir -p "$tmp/old/crates/service-k8s/examples"
cp "$root/crates/service-k8s/examples/render_dump.rs" "$tmp/old/crates/service-k8s/examples/render_dump.rs"

if ! (cd "$tmp/old" && CARGO_TARGET_DIR="${UPGRADE_ROLLBACK_TARGET_DIR:-$tmp/target}" \
  cargo run --quiet --locked -p service-k8s --example render_dump) > "$out.partial"; then
  rm -f "$out.partial"
  echo "FAIL: cargo run render_dump failed at $tag" >&2
  exit 1
fi

mv "$out.partial" "$out"
echo "old render of $tag written to $out"
