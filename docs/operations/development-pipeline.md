# Normal development pipeline

Core adopts [AW DEV-PIPELINE-1](https://github.com/faberline/aw/blob/68b2d295ee9bf874d560f6054fa993e164464d3e/docs/standards/development-pipeline.md)
with shared runtime `8b8a5284ef928190c0d21718331de4b1732334c9`.
The repository adoption issue is [#84](https://github.com/faberline/core/issues/84).
Workflow publication alone does not complete protected policy adoption.

Ordinary PRs use `.github/workflows/development-ci.yml`:

1. `development-preflight` checks the frozen head/base, current main ancestry,
   mergeability, and exactly one canonical open issue.
2. `unit-plan` records changed, deleted and renamed paths and reverse
   dependency impact. It still selects the complete current workspace and
   feature matrix. The impact report cannot remove a required test.
3. Formatting, unit/static and issue-contract inspection complete first.
4. Every original package/Raft/contract check and the measured-performance
   job wait for all stage 3 jobs. Each job keeps its own cloud runner.
5. The pinned read-only Claude producer runs only after complete required
   qualified CI. It must bind the same source issue and exact commits.
6. The trusted result writer rechecks all required evidence and provider
   rules before merge. The actual merge must be verified before issue closure.

The reviewed policy retains all 144 existing expanded checks and adds the
preflight check, for **145 required checks**. No performance workload or
threshold is changed. The old `.github/workflows/ci.yml` retains its existing
main/tag/manual execution; it no longer duplicates ordinary PR tests.
Release pipeline design remains outside this change.

Each failed nonfinal job gets an issue comment with its observed failure,
failed step and direct evidence links. One serialized writer performs the
current coding attempt's `todo` handoff. Later matrix failures add context
without resetting an agent that already claimed that handoff. Missing issue
links are diagnosed on the PR instead of guessing an issue.

Main updates and PR/issue events wake the same result writer. Incomplete,
skipped, stale, unknown or unreviewed evidence remains blocked. Protected
workflow and policy edits retain the separate adoption process. A green
writer job is not proof of product CI, review, merge or issue closure.

`scripts/issue-ci` contains pinned copies for offline inspection. Privileged
execution uses the shared AW actions. The planner's Python parser uses the
hash-locked AW dependency file in a disposable runner environment.
