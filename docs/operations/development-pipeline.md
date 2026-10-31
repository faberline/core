# Normal development pipeline

Core adopts [AW DEV-PIPELINE-1](https://github.com/faberline/aw/blob/4223a4534f0197e1ea23004b9388ffd1eea34639/docs/standards/development-pipeline.md)
with shared runtime `4223a4534f0197e1ea23004b9388ffd1eea34639`.
The repository adoption issue is [#84](https://github.com/faberline/core/issues/84).
Workflow publication alone does not complete protected policy adoption.
AW's exact shared source passed all 1,175 controls with zero skips. Core
adoption still needs current-head product CI, independent local review and
actual merge evidence. The actor inventory and ledger/history remain
preserved. Historical [guarded completion](guarded-completion.md) is retired
as an activation prerequisite for normal local completion.

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
5. CI ends after tests. Its single label writer posts reasons and returns the
   current failed attempt to todo, or labels the open issue done only after
   complete qualified CI. It has issue-write permission and read-only access
   elsewhere; it cannot review, merge or close an issue.
6. Independent local review runs after required current-head CI.
   The authorized local controller verifies the exact merge before closing
   the canonical issue. Keep strict base and every required test check.

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
The same test implementation also handles release tags in `all` mode.
PRs request `diff`; the current reviewed Core policy requires the whole
inventory for every path, so it retains a conservative full fallback.
`scripts/pr-gates/selection.py` loads the pinned AW selector and records
exact head/base, policy and complete plan digest. Protected proposals execute
every old gate but keep `pending_protected_review`; a plan grants no approval.
See [release entry](release-pipeline.md) for the tag and milestone boundary.

Each failed nonfinal job gets an issue comment with its observed failure,
failed step and direct evidence links. One serialized writer performs the
current coding attempt's `todo` handoff. Later matrix failures add context
without resetting an agent that already claimed that handoff. Missing issue
links are diagnosed on the PR instead of guessing an issue.

Main updates, issue events and test-run events wake the same result writer. Incomplete,
skipped, stale, unknown or unreviewed evidence remains blocked. Protected
workflow and policy edits retain the separate adoption process. A green
writer job is not proof of product CI, review, merge or issue closure.

`scripts/issue-ci` contains pinned copies for offline inspection. Privileged
execution uses the shared AW actions. The planner's Python parser uses the
hash-locked AW dependency file in a disposable runner environment.

The Core planner profile is shared: triage, queue and release assessment.
App planners originate product capability demand and link their source issue
and milestone. Core does not invent a separate product roadmap. The existing
core@0.5.0 milestone is explicitly a work batch and Cargo still says 0.4.14.
The empty release mapping deliberately holds tagging until a reviewed goal,
version and acceptance mapping exists. Do not guess or bump package versions.

Shared AW #148 fixes same-head CI reruns. After local review returns a head,
its reruns do not restore done or reset a new doing owner. Later failed jobs
still get diagnostics. A new submitted head, unclaimed infrastructure retry
and unfinished journal retain the normal guarded path. Old policy receipts
prove a handoff only; they do not qualify current tests.

Shared AW #150 also records failed-job diagnostics for a valid open canonical
issue without lifecycle labels. Unsafe lifecycle handoff remains incomplete;
stale inputs, duplicate events and later owners retain their protections.

AW #151 binds diagnostics and labels to the verified real canonical issue.
A Closes link to another PR, a closed issue or a foreign item cannot receive
issue handoff writes. Invalid links are diagnosed only on the source PR.
Recheck target identity before each write, including issue-to-PR conversion.

AW #152 reports confirmed missing target 404 responses on the source PR.
Authorization, rate-limit, server, network and unknown responses remain
held. Final writer checks reject changed links or newly valid targets.
This adoption uses the user-authorized independent Codex review in
place of the older Claude tool requirement; all independence and
current-input gates remain.

AW #153 separates source-PR comments from the single issue-label writer.
The source-pr-links helper has pull-requests write, issues read and
contents read. It can only comment on the exact source PR. The report
job keeps issues write, pull-requests read and contents read. It runs
after the helper with always(), under the same workflow mutex. Both
roles bind GITHUB_JOB and reject review, merge, close, status and
unrelated writes. No product test or numeric limit changes.
