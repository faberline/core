# Guarded completion preparation

Core consumes the [AW activation contract](https://github.com/faberline/aw/blob/fb002d9198967313a4641f129a9c9648890a7555/docs/standards/issue-guard-activation.md).
Issue #86 prepares the inactive deployment. Issue #88 stages the reviewed
failure-reason runtime before the final activation. Issues #83 and #84 remain open
until their full deployed acceptance is proved. AW #115 also remains open.

The policy contains a stable drain nonce and disabled version 2 recovery.
The new runtime keeps the existing product commands and thresholds. It adds
readable authenticated rejection reasons from AW #138. Core must authenticate
a new deployed drain and inactive recovery at this source before activation.
It contains neither `issue_guard` nor `issue_guard_rollout`. The trusted writer
withdraws native merge requests during drain. Guard-capable workers refuse new
claims. Old launcher fencing and in-flight work still require an actual audit.
An old session status or an absent local lock is not proof that work stopped.

The journal policy pins the same reviewed AW runtime as this deployment.
The six-stage test plan changes the deployed CI semantics, so its journal
policy has a new identity. The earlier `8b8a5284` value was an undeployed
intermediate revision, not the existing main journal identity. It cannot
stand in for either the deployed base or the reviewed current runtime.

This preparation does not rewrite, delete or replay any existing journal.
During drain, the trusted writer returns through `drain_queue` before it can
call the ordinary CI handoff. Existing issue comments and leases stay intact.
The later activation must settle old work and leases before enabling the
guarded writer. If a later change adds review metadata without changing CI
semantics, the verified deployed journal policy can retain its old identity.

The recovery workflow ID is **402173927**. GitHub registered this real ID from
the isolated `codex/86-register-recovery` branch. That registration has no
provider permission, product checkout, tests or recovery operation. Its result
is not completion evidence. The final workflow definition has its own exact
blob in the policy. Its only triggers are writer completion and an explicit
recovery wake. It adds no schedule. The authenticated disabled entry returns
an inactive result and cannot run recovery mutations.

The actor manifest declares these required deployment audit targets:

| Actor | Audit required |
| --- | --- |
| `claude-core-project-issue-work` | Project plugin entry and any old loaded entry paths |
| `agy-core-issue-work` | Actual AGY launchers, including unknown or remote entries |
| `core-issue-ci-writer` | Default-branch writer ID, source, mutex and runtime |
| `core-issue-recovery` | Default-branch recovery ID, source, mutex and runtime |
| `codex-pipeline-controller` | Current controller, manual claims and supported entry paths |
| `core-issue-plan-controller` | Existing planner issue/label writes and their actual launchers |

The manifest is a list of audit targets. It is not an actor audit or a claim
that these entries are fenced or idle. Resolve unknown launchers before the
manifest can be called complete. Preserve Core #83's preparation worktree,
PR #82, PR #85, current owners and all schedules.

After the inactive preparation merges, read back the actual drain output.
Audit and fence the supported launchers with the exact approved worker,
provider, ledger and entry runtime. Account for every old lease and in-flight
operation. Confirm the guard namespace and no-bypass provider rules. Keep
activation held while an actor or result is unknown.

Publish actual evidence in AW's data-only rollout registry. The final Core
candidate must bind that immutable source and the same nonce. Require the
complete current-head CI and independent Claude PASS again before guarded
adoption. The current preparation does not supply those future receipts.

Do not close the normal pipeline adoption issues on a queued merge, a passing
registration job, or a passing inactive recovery job. Verify the actual merge,
canonical issue close/readback and safe close-only retry in the final deployed
integration. Keep all 145 checks and the original performance bounds. Core
owns its batch layout; AW sets no shared job count.
