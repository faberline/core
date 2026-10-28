# Publication candidate and CI execution scope

Refs [#73](https://github.com/faberline/core/issues/73).

## Reviewable local candidate

- Branch: `codex/73-pr-unit-static-preparation`.
- Worktree: `/tmp/core-73-pr-gates.li3y5I/core`.
- Base: `41c352eaae687a1914087d279f36adbe9e9b4af5`.
- State: prepared for the human-authorized draft PR publication on 2026-10-28.
  No manual workflow dispatch, paid review or merge. The original prepared
  worktree remains unchanged at `/Users/chrischeng/faberlines/core-73-unit-static`.
- Independent verdict: [READY for proposal publication](independent-review.md),
  with bootstrap/qualification/merge BLOCKED.
- Evidence: [local verification](local-verification.md),
  [workspace and 22-PR inventory](README.md).

The full diff includes the workflow, planner, both fixture files, and all
qualification inventory/review documents. It excludes product code, Cargo
manifests, #51's tree, the required-gate policy and controller changes.

Suggested proposal title: `ci: propose complete workspace unit/static gates with fail-closed bootstrap`.
Use `Refs #73`; current local evidence does not justify closing the issue.

## Consequences of an authorized proposal publication

The existing `pull_request` trigger normally runs the original five crate jobs.
This candidate adds one PR-only planning job. Since its base has no trusted
driver, that job is expected to fail with the explicit bootstrap message,
and the dependent 70-row product matrix will not launch. This deliberate
failure must not be bypassed by falling back to PR-owned helper code.

If a separately reviewed bootstrap and immutable identity contract are later
approved, the current inventory schedules:

- One planning/fixture/inventory/format job.
- 70 package/profile jobs, each running one current library unit command and
  one all-target clippy command: 140 Cargo commands plus workspace format.
- At most four matrix jobs concurrently; each new job has a 30-minute timeout.
- The original five crate jobs remain separate and unchanged.

Thus a fully enabled current PR has 71 additional jobs (76 including the five
existing jobs). Configured new-job timeout budgets sum to 2,130 runner-minutes
at the current row count; that is a timeout-budget calculation, not measured
runtime, billed minutes or a monetary estimate. Queue time, setup, actual
runner billing and failure timing are unmeasured. Additional binary targets
can add unit invocations and feature changes can alter row counts. The planner
refuses over 256 matrix rows rather than dropping them.

No runners, paid capacity, GKE resources, secrets, credentials, permissions,
schedules or privileged event triggers are added. New checks are PR-only;
push/tag/manual behavior of the original five jobs is unchanged.

## Outstanding approvals and evidence

Proposal publication is separate from bootstrap activation. Formal adoption
requires externally approved immutable workflow/driver/evaluator identity,
not a PR job name or a self-reported green result. Existing failure-only holds
remain. Complete online required testing, zero-test applicability where needed,
issue-specific assertions, functional/e2e/performance mapping and measured
results, final Claude review and authorized delivery are still required.

The human authorized committing and publishing the coverage work on 2026-10-28.
The draft publication does not approve bootstrap, merge or product acceptance.
This controller revalidated 43 gate controls and 10 existing controller controls;
all 53 passed. Fresh offline metadata still derives 70 rows across 30 library
targets. Driver and workflow SHA-256 values match the previous independent
review. No product suite ran locally; the initial online bootstrap refusal is
expected and must remain visible.
