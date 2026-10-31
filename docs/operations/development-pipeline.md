# Normal development pipeline

Core adopts [AW DEV-PIPELINE-1](https://github.com/faberline/aw/blob/fb002d9198967313a4641f129a9c9648890a7555/docs/standards/development-pipeline.md)
with shared runtime `fb002d9198967313a4641f129a9c9648890a7555`.
The repository adoption issue is [#84](https://github.com/faberline/core/issues/84).
Workflow publication alone does not complete protected policy adoption.
The inactive completion preparation is tracked separately by
[#86](https://github.com/faberline/core/issues/86). Its drain blocks normal
merge requests. It does not complete #83, #84, or automatic issue closure.
See [guarded completion](guarded-completion.md) before enabling the guard.

Ordinary PRs use `.github/workflows/development-ci.yml`:

1. `development-preflight` checks the frozen head/base, current main ancestry,
   mergeability, and exactly one canonical open issue.
2. `unit-plan` records changed, deleted and renamed paths and reverse
   dependency impact. It still selects the complete current workspace and
   feature matrix. The impact report cannot remove a required test.
3. One static job runs formatting and issue-contract inspection. Eight unit
   batches run all 70 package/feature rows, each with its original unit,
   all-target Clippy, and documentation commands.
4. Eight functional batches retain all 62 contract rows, five original crate
   checks, and three issue 47 checks. They and the isolated performance job
   wait for every stage 3 job. Each live Kubernetes contract case gets a new
   disposable Kind cluster, even when several cases share a batch.
5. The pinned read-only Claude producer runs only after complete required
   qualified CI. It must bind the same source issue and exact commits.
6. The trusted result writer rechecks all required evidence and provider
   rules before merge. The actual merge must be verified before issue closure.

The reviewed policy retains all 144 existing expanded checks and adds the
preflight check, for **145 required checks in 20 physical test jobs**.
Core owns the batch counts in `.github/development-test-batches.json`. AW
defines stages and success barriers and sets no common job or batch count.
The runner derives the complete case roster from fresh Cargo metadata. It
checks that roster against the reviewed policy, logs each case result and
the complete batch receipt, and fails on any missing, duplicate or failed
case. Each feature row keeps its own original command. No performance workload or
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
