# Core release entry

Since #140, this repository's `.github/workflows/ci.yml` runs no release preflight
and no performance threshold. Both come back with the release workflow, which is follow-up work.

Release uses `.github/workflows/ci.yml` (#140 replaced the old development workflow).
Every `v*` RC or formal tag must pass AW's immutable tag/milestone preflight
and select `all`. It executes every Core-owned required test and original
numeric threshold. Independent legacy main/tag/manual gates remain intact.

The shared [release contract](https://github.com/faberline/aw/blob/4223a4534f0197e1ea23004b9388ffd1eea34639/docs/standards/release-pipeline.md)
defines the lifecycle. Issue-plan checks the user-defined milestone, closed
issues, landed PRs and actual goal evidence. It fills real gaps before an RC.
Full-test failure creates a same-milestone backlog repair with evidence. The
planner adds exact affected unit commands before guarded todo admission.
Repair and merge precede the next RC. Preserve tags and all test thresholds.

Only the exact passing RC commit may receive the final tag. That tag also runs
all tests. Publication must succeed before milestone closure. A formal-tag
code repair needs a new reviewed SemVer version. Cloud CI reviews or merges
no PR and closes no issue.

`.github/aw-release.json` holds the reviewed contract for milestone 8:
`core@0.5.0` is version `0.5.0`, tag `v0.5.0`, and a milestone is ready when
all its issues are closed. The release-contract check script was removed by
#140, so no command prints the holds. The workspace version is `0.5.0` (bumped
by #156), which matches the milestone version. The tag still waits for the
closed milestone and a user decision. This change creates no tag and starts no
release.

Use the installed shared skill's absolute `scripts/plan.sh` path for
`release-status --milestone 8`. Only a fresh matching assessment permits its
guarded `release-advance` operation. Reconcile unknown provider responses
before retry. A tag alone is not proof that full tests ran.

A frozen passing RC can proceed after unrelated main work advances. Its exact
commit must still be in main ancestry and include every related milestone PR
and the current reviewed version/goal contract. Tag preflight uses that tagged
source and plan. A child PR counts only once its merge commit actually lands
in main through its parent. This does not remove the empty-mapping hold.
