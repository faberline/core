# Proposed selection and aggregation fixtures

Refs [#73](https://github.com/faberline/core/issues/73). These are acceptance
proposals for a future reviewed qualification policy. The unit/static planner
does not implement a passing product-qualification evaluator.

Use pure in-memory inputs. Start with an exact-head evidence fixture naming
repository, head/base, trusted workflow/policy revision, job/matrix, run/attempt,
command/profile, named tests and counts, immutable output digest, applicable
oracles and reviewed applicability. Mutate one field per negative control.

| Fixture | Required result / oracle |
|---|---|
| Change a low-level local dependency | Include every transitive reverse dependent, including optional, dev and build edges; shuffled inputs produce the same sorted set. |
| Change one crate's source | Include that crate and dependents; execution still uses the complete proposed unit/static matrix. |
| Root Cargo.toml, Cargo.lock, unknown/deleted/renamed crate path, empty change list | Select complete workspace; no empty selection passes. |
| Docs-only PR | No blanket zero-check pass; require the full proposed unit/static matrix or an independently reviewed explicit applicability policy. |
| Missing workspace member, duplicate identity, incomplete manifest or graph | Fail inventory; never qualify a partial set. |
| New binary or library target with test=false | Require an explicit tested harness/matrix decision; never silently exclude it. |
| Missing expected job, skipped, cancelled, pending, neutral or timed-out job | Nonqualified, even if other matrix rows pass. |
| Success for stale head, wrong repository, wrong workflow path or policy revision | Nonqualified; visible green checks do not substitute for immutable identity. |
| Old successful attempt followed by current failed/pending attempt | Nonqualified; use latest current attempt. |
| Required feature profile absent, wrong feature arguments, zero matching issue rows | Nonqualified until roster and reviewed applicability prove the required behavior ran. |
| Unit/static pass, functional/e2e mapping absent | Nonqualified; distinct obligations cannot substitute for one another. |
| Performance job exits zero without measured workload, baseline, threshold or result digest | Nonqualified; unmeasured performance is not a pass. |
| Claimed not-applicable without explicit approved rationale/reference | Nonqualified; unknown oracle/owner/applicability remains a blocker. |
| All jobs green, failure-only mode or nonempty qualification_blockers | No done/merge authorization and no lifecycle write. |
| Genuine complete evidence with approved full policy | Eligible only for independent review and separately authorized delivery; fixture does not merge, close an issue or waive final Claude review. |

Fixture outputs must include the failing obligation and evidence identity so a
controller success cannot hide the reason a PR is still pending. A positive
case is meaningful only after the policy and actual command/oracle mappings
receive review. The existing failure-only policy remains untouched meanwhile.
