#!/usr/bin/env bash
# Shape check for .github/workflows/release.yml (issue #225, part 2 of #223).
# Usage: bash ci/check-release-shape.sh [release.yml [ci.yml]]
set -euo pipefail

release="${1:-.github/workflows/release.yml}"
ci="${2:-.github/workflows/ci.yml}"
fail=0

report() {
  echo "FAIL: $1" >&2
  fail=1
}

for f in "$release" "$ci"; do
  if [ ! -f "$f" ]; then
    echo "FAIL: missing $f" >&2
    exit 1
  fi
done

# Workflow name must be exactly `release`.
grep -qxF 'name: release' "$release" || report "workflow name is not release"

# Both tag patterns must be present, as literal text.
grep -qF '"v[0-9]+.[0-9]+.[0-9]+"' "$release" || report "final tag pattern is missing"
grep -qF '"v[0-9]+.[0-9]+.[0-9]+-rc.[0-9]+"' "$release" || report "RC tag pattern is missing"

# No filter that would narrow the tag trigger.
for key in 'paths:' 'paths-ignore:' 'tags-ignore:' 'branches:'; do
  if grep -qF "$key" "$release"; then
    report "release.yml has $key"
  fi
done
if grep -qE '^[[:space:]]*- ["'"'"']!' "$release"; then
  report "release.yml has a ! tag pattern"
fi

# Job ids are the two-space-indented keys under `jobs:`.
job_ids=$(awk '/^jobs:/{injobs=1; next} injobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/{gsub(/[ :]/,""); print}' "$release")
for id in preflight ci perf-thresholds gate; do
  grep -qx "$id" <<<"$job_ids" || report "no job id $id"
done

# Text of one job: from its `  <id>:` line to the next job id.
job_block() {
  awk -v id="$1" '
    $0 == "  " id ":" {on=1; next}
    on && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {on=0}
    on {print}' "$release"
}

preflight=$(job_block preflight)
grep -qF 'GITHUB_SHA' <<<"$preflight" || report "preflight does not compare the tag commit with GITHUB_SHA"
grep -qF 'matching-refs/heads' <<<"$preflight" || report "preflight does not check for a branch with the tag name"

ci_job=$(job_block ci)
grep -qF 'uses: ./.github/workflows/ci.yml' <<<"$ci_job" || report "ci job does not call ci.yml"

perf=$(job_block perf-thresholds)
grep -qF 'python3 ci/measure-performance.py' <<<"$perf" || report "perf-thresholds does not run ci/measure-performance.py"

gate=$(job_block gate)
grep -qE '^    if: .*always\(\)' <<<"$gate" || report "gate does not have if: always()"
gate_needs=$(sed -n 's/^    needs: *//p' <<<"$gate")
for id in preflight ci perf-thresholds; do
  grep -qE "(^|[][, ])${id}([],]|\$)" <<<"$gate_needs" || report "gate does not need $id"
done
grep -qF "'skipped'" <<<"$gate" || report "gate does not reject a skipped job"

# ci.yml must stay callable through workflow_call.
grep -qxF '  workflow_call:' "$ci" || report "ci.yml has no workflow_call trigger"

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "release.yml shape ok: $release"
