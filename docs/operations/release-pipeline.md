# Core release procedure

## What runs

A tag push `vX.Y.Z-rc.N` or `vX.Y.Z` starts `.github/workflows/release.yml`.
It runs these jobs:

- `preflight`: checks the tag against its commit and the reviewed contract.
- `ci`: every job of `ci.yml`, called through `workflow_call`. Each job shows
  as `ci / <job name>`.
- `perf: thresholds`: runs `ci/measure-performance.py` with the original thresholds.
- `gate`: passes only when every job above passed.

## The two reviewed files

- `.github/aw-release-gates.json` is the policy. It names the workflow, its
  id, its blob hash (`definition_sha`) and each job name that counts, with its kind.
- `.github/aw-release.json` is the contract per milestone. It holds the
  acceptance goals, the closed issues and the checks for each goal.

A change to `release.yml` changes its blob hash. The same pull request must
update `definition_sha`. `ci/check-release-policy.py`, run by the `lint` job,
fails otherwise.

## Release core@0.5.0 (milestone 8)

1. Run `aw release-status --milestone 8`. Read `state`, `action` and `fingerprint`.
2. Run `aw release-advance --milestone 8 --expected <fingerprint>`. This creates
   the RC tag `v0.5.0-rc.1` on the commit it names.
3. The tag push runs `release.yml` as full qualification on the RC commit.
4. Run `aw release-preflight --tag <tag> --commit <sha>`. This checks the tag
   against its commit and the contract.
5. Only when every required job of the RC run succeeded, `aw release-status --milestone 8`
   allows the final tag. Then `aw release-advance --milestone 8 --expected <fingerprint>`
   creates `v0.5.0` on the same commit, and the same workflow runs again.
6. A person publishes the GitHub release and closes the milestone.

The commands `aw release-status`, `aw release-advance` and `aw release-preflight`
come from faberline/aw#284. Check `aw --help` before you use them.

## Rules

- A missing, skipped, cancelled, stale or unknown check is not a pass.
- Do not lower a test, a matrix row or a threshold. A change to any of them
  needs a new reviewed version.
- A job that is skipped on a tag kind can never pass for that kind.
- Creating a tag, an RC or a release is a person's decision. This document does not start one.
