# Historical independent code and issue-fit review

> Historical review only. The workflow and planner changed on 2026-10-28.
> This verdict does not cover the current PR head. Fresh independent review
> is required. The former bootstrap restriction now applies only to future
> trusted qualification, not ordinary read-only PR tests.

Refs [#73](https://github.com/faberline/core/issues/73). Read-only review and
re-review were performed by the native Codex subtask
`core73_independent_review`, separately from the implementing subtask
`core73_gates`. No standalone Codex/AGY CLI, paid review, product suite or CI
was used. This is not the final Claude review required for delivery.

## Verdict

**READY for explicit proposal publication and review. Product qualification,
bootstrap activation and merge remain BLOCKED.**

Reviewed final source identities:

- `scripts/pr-gates/plan.py` SHA-256:
  `4afb5fc0a833d13460fb50ddc9efb52e27d40c05ec381303145059ba4c4473ce`
- `.github/workflows/ci.yml` SHA-256:
  `1827f8b35fc534b05fc31d036a018175390c04089e1a8c100475091189c125be`

## Findings and corrections

| Finding | Candidate correction | Remaining boundary |
|---|---|---|
| PR helper/fixtures can emit a smaller matrix or exit zero | Execute only the event base driver and a nonempty baseline fixture suite; compare driver/workflow identities and refuse missing bootstrap | An editable PR workflow can still replace those steps. Trusted adoption must independently bind approved immutable workflow/driver/evaluator identities; no policy activation occurs here |
| Cargo metadata `test=true` does not prove `harness=true` | Read actual lib/bin manifest declarations; reject disabled/custom harnesses, with matching fixtures | No custom-harness waiver is installed |
| Exit zero with zero tests was accepted | Execute each active lib/bin separately; require a nonempty matching named roster and complete successful counts; reject zero, ignored, filtered, measured, duplicate or partial records | A named successful test does not prove semantic assertions or issue fit; independent source/oracle review remains required |
| HEAD/base and baseline inventory drift were not bound | Verify event expected identities through Git argv, recheck before product execution and compare fresh base/head inventories; reductions refuse | New feature combinations and issue-specific assertions still require explicit reviewed applicability and online evidence |

The reviewer independently ran 43 helper fixtures and 10 existing controller
fixtures, all passing. YAML parsing, original job/envelope preservation,
PR-only additions and diff whitespace checks passed. The old five crate jobs,
failure-only policy, qualification blockers and controller bytes remain intact.
Permissions remain `contents: read`; no privileged trigger, secrets, resource,
credential or lifecycle writes were added.

## Issue-fit limits

The candidate inventories all 30 current library targets and supports future
binary targets; its 70 package/profile rows are a plan, not test-success proof.
The feature policy covers default/no-default, each effective feature and
all-features. It does not prove every feature-subset interaction. #47/#51's
explicit proposal/full-suite/clippy gates and all other original issue scopes
remain separate. The 22 PR inventories retain unknown performance/applicability
as blockers and do not close partial issues.

The current base has no driver, so an initial proposal PR is expected to fail
the bootstrap guard before running the new product matrix. Actual online
current-head results, approved functional/performance oracles, complete trusted
qualification policy, independent bootstrap approval, final Claude review and
authorized delivery remain outstanding. READY above authorizes no action.
