# Core PR unit/static gates and qualification inventory

Refs [#73](https://github.com/faberline/core/issues/73). This is a bounded
unit/static implementation and evidence inventory. It does not clear the
existing qualification holds, complete any feature issue, or authorize online
execution, delivery, merge, release or lifecycle changes.

## Trust and bootstrap boundary

The new checks are **candidate checks, not trusted qualification**. An ordinary
PR can edit its workflow, so a green job name alone is not admissible proof.
A future trusted evaluator must independently bind the approved immutable
workflow and driver revisions, complete expected matrix and current HEAD/base.
This change does not install that evaluator or amend its failure-only policy.
See the [independent review](independent-review.md),
[publication/CI scope](publication-candidate.md), and
[local verification](local-verification.md) for the candidate verdict and limits.

The proposed PR jobs check out the event's base and head in separate directories
with `persist-credentials: false`. Only the base copy supplies the driver and
its nonempty fixture suite; the candidate helper is never a fallback. Driver
or workflow changes refuse self-qualification. Runtime identity checks compare
both checkouts with the event's expected HEAD/base. Baseline inventory removals
or feature/target reductions require a separate reviewed decision and fail here.
The base driver remains unprivileged: `contents: read`, no added secrets,
privileged event trigger, credential, infrastructure or lifecycle writer.

At this candidate's base `41c352e...`, `scripts/pr-gates/` does not yet exist.
Therefore the initial candidate is **BLOCKED for automatic bootstrap**: its new
planning job will fail before product execution instead of running PR-owned
gate code. Independently reviewed bootstrap of an immutable workflow/driver
identity, and separately authorized online execution, remain explicit delivery
steps. Publishing a proposal for review does not approve that bootstrap.

## Inventory and provenance

- [Workspace inventory](workspace-inventory.json): Cargo manifests, features,
  local dependency edges and actual Cargo targets at baseline
  `41c352eaae687a1914087d279f36adbe9e9b4af5`.
- [PR index](pull-requests.json): 22 known open PRs, each linked to a small
  [per-PR snapshot](pull-requests/) with exact head/base, changed paths,
  changed crates, reverse dependents, source issue links and remaining gates.
- GitHub metadata was read on **2026-10-27 09:36:04 UTC** using
  `get_pr_info`, paginated `list_pr_changed_filenames` and `fetch_issue`.
  The source issue body digest and update time identify the read contract;
  the linked issue remains authoritative if it changes.
- `cargo metadata --locked --no-deps --offline --format-version 1` read the
  baseline workspace without compiling or running product tests. Source and
  manifest digests identify the inventoried bytes. This snapshot is preparation,
  not exact-head test evidence. Refresh it after any PR head, manifest, feature,
  test roster or dependency graph changes.

The baseline contains **30 library targets, 0 binary targets, 29 integration
test targets, 3 examples, and 0 Cargo bench targets**. Ten crates declare
features. The effective Cargo feature inventory also includes the implicit
`ui-runtime/serde_json` feature. The machine inventory includes every target,
its `test`/`doctest` flag, required features and test-module paths. All libraries
have unit harnesses enabled; there are no disabled library/binary targets to
waive. The three example targets have `test=false` and are outside the requested
library/binary unit gate. Their omission is not functional acceptance.

## Executable unit/static matrix

`scripts/pr-gates/plan.py` derives execution from current Cargo metadata and
checks it against the workspace member manifests. The workflow preserves the
five existing crate jobs and adds unit/static jobs over the complete inventory.
It does not install a trusted qualification policy or alter
`.github/issue-ci-required-gates.json`.

Each package runs independently (`-p`), preventing workspace feature unification
from hiding missing feature guards as warned by `CONTRIBUTING.md`. Every package
gets its default profile. Each feature-bearing package additionally gets
`--no-default-features`, one `--no-default-features --features <feature>` row
for every non-default effective Cargo feature, and `--all-features`.
At this baseline that produces **70 package/profile rows**. Manifest feature
edges and exact row names are recorded in the workspace inventory; the runtime
plan is regenerated rather than accepting this historical count as proof.
Each row runs unit tests and all-target clippy, totaling 140 proposed Cargo
commands plus workspace formatting. The matrix limits parallel execution to
four rows. These commands have not been dispatched by this change. A future
package with multiple lib/bin targets runs one unit invocation per enabled
target, so its command count will increase. New checks apply only to PRs;
existing push/tag/manual trigger behavior and five crate jobs stay unchanged.
There is no documented mutually exclusive feature pair in the current manifests;
an incompatible combination must fail and receive an explicit reviewed matrix
decision, never a silently dropped row.
This covers each declared feature individually plus default and all-features;
it does not enumerate the feature power set or prove every interacting subset.
An issue that requires a specific combination must retain that additional gate.

The commands are:

```sh
# Planning/static metadata only; safe locally, no product tests:
cargo metadata --locked --no-deps --offline --format-version 1 > /tmp/core-metadata.json
python3 scripts/pr-gates/plan.py inventory --metadata-json /tmp/core-metadata.json
python3 scripts/pr-gates/plan.py matrix --metadata-json /tmp/core-metadata.json

# Product execution belongs to the normal authorized online PR path:
cargo test --locked -p <package> --lib <feature-arguments>
cargo clippy --locked -p <package> --all-targets <feature-arguments> -- -D warnings
cargo fmt --all -- --check
```

If binary targets appear, the derived plan includes each enabled target by
`--bin <name>` alongside `--lib` where present. Missing
members or disabled/custom library/binary harnesses fail planning. Each enabled
target must produce a nonempty named libtest roster and matching successful
counts, without ignored, measured or filtered rows. A zero-test target blocks
execution qualification pending separately reviewed applicability; there is no
automatic waiver. Roster counts do not prove semantic assertion coverage or
issue fit. The unit gate deliberately excludes
`tests/it`/other integration targets and therefore does not replace their gates.
No unfiltered product tests, compilation, integration/e2e or performance suite
was run locally for this inventory.
The helper requires Python 3.11 or newer. For an offline local inspection, save
the metadata command above and pass its JSON file with `--metadata-json` to
`inventory` or `matrix`. Product `run` refuses saved metadata and reads the
current checkout plus the baseline instead. Every execution requires explicit
expected HEAD/base identities and records them with the target/profile result.

## Change selection and conservative fallback

The affected-crate map is advisory. Execution in this proposal is the complete
workspace matrix, including docs-only PRs. The map associates a crate path with
its owner and computes the transitive reverse closure across normal, dev,
build and optional local dependencies. Unknown paths, root configuration,
Cargo.lock, deleted/unrecognized crates and empty path inventories fall back
to all workspace crates. Renames need both old and new paths; if either cannot
be resolved, use the same full fallback. Optional dependencies stay in the
closure even when disabled by one feature profile.

Per-PR closures in this directory use the baseline graph. PRs #53 and #67 change
Cargo.lock and therefore select all 30 crates. Before qualification, head
metadata must be compared with base metadata; a new edge, removed crate, changed
workspace glob or incomplete graph cannot inherit a smaller historical closure.
Docs-only changes have no approved zero-check exemption: formatting, metadata
and the full proposed unit/static matrix still run, while any future narrower
applicability rule needs reviewed rationale and immutable policy identity.

## Qualification remains blocked

The [existing trusted policy](https://github.com/faberline/core/blob/main/.github/issue-ci-required-gates.json)
uses `failure-only`. Its five fixed jobs cover `service-k8s`, `storage-durable`,
`service-backup` (http-client), `raft-core` and `raft-runtime`; that is not a
general workspace feature/target policy. The successful
[Core CI run 40423429926](https://github.com/faberline/core/actions/runs/40423429926)
for PR #51 head `019061dd744aaa8386d77246d5f9b61bf3f5bef4` establishes only
those five jobs. The [handoff job](https://github.com/faberline/core/actions/runs/40423896447/job/121924770170)
reported all 22 heads pending. Controller success is not product qualification.

For [#47 / PR #51](https://github.com/faberline/core/pull/51), preserve all
three explicit source gates, including proposal behavior and clippy:

```sh
cargo test --locked -p raft-runtime --test it proposal
cargo test --locked -p raft-core -p raft-runtime --no-fail-fast
cargo clippy --locked -p raft-runtime --all-targets -- -D warnings
```

Their oracle is the source issue's same-term completion, prompt superseded-term
ambiguity, unchanged existing rows and negative control. Unit/static success
does not prove the host-level regression or the full existing suite; the
proposal filter's actual named roster and counts must be recorded. No current
head clippy pass is asserted by this document.

The retained implementation session reported a historical clippy failure at
`raft-core/conf_adoption.rs:26`. It was not rerun by this inventory, so it remains
a reported blocker requiring exact-head evidence rather than a newly reproduced
failure or a verified pass.

Each PR snapshot lists existing integration target commands as candidates and
links the baseline target source. These commands must run online and be mapped
against the complete source issue acceptance, affected dependents and any
downstream consumer/cluster behavior. A target name does not by itself prove
functional/e2e completeness. Legacy issue commands referencing `libs/` or
absent `scripts/meta/` paths remain unmapped blockers rather than being executed
or treated as passes under the split repository layout.

No reviewed affected-performance command, workload, environment, baseline or
threshold is established for these PRs. Zero Cargo bench targets does not waive
performance: retain a blocked measured-performance obligation for every PR
until its oracle and applicability are approved. The five-milestone line
coverage target of at least 99% also has **no verified measured baseline** here;
no coverage achievement is claimed.

Required evidence identifies repository, exact PR head/base, immutable policy
and workflow revision, job/matrix, run and attempt, command/profile, named test
roster/counts, oracle version, immutable result digest, and applicability review.
The stages remain distinct: proposal approval, implementation, authorized online
testing, independent review, then separately authorized delivery. After all
required checks pass, the final Claude review must check that the result meets
the issue exactly. No issue is closed by this inventory.

## Bounded follow-ups on existing issues

| Existing issue | Remaining work boundary |
|---|---|
| [#73](https://github.com/faberline/core/issues/73) | Review and validate the proposed unit/static matrix, exact-head inventory/selection, functional and measured-performance command/oracle mapping, and qualification policy before changing holds. |
| [#47](https://github.com/faberline/core/issues/47), [#48](https://github.com/faberline/core/issues/48), [#49](https://github.com/faberline/core/issues/49) | Obtain their explicit raft gates and independent review on their own current PR heads; preserve negative controls and unchanged existing tests. |
| [#15](https://github.com/faberline/core/issues/15), [#16](https://github.com/faberline/core/issues/16), [#17](https://github.com/faberline/core/issues/17), [#18](https://github.com/faberline/core/issues/18), [#19](https://github.com/faberline/core/issues/19), [#26](https://github.com/faberline/core/issues/26), [#27](https://github.com/faberline/core/issues/27), [#28](https://github.com/faberline/core/issues/28), [#29](https://github.com/faberline/core/issues/29), [#30](https://github.com/faberline/core/issues/30), [#31](https://github.com/faberline/core/issues/31) | Reconcile frozen migration acceptance and real current-path functional/integration rows; a model, rendering primitive or partial PR is not completion of the source issue. |
| [#32](https://github.com/faberline/core/issues/32), [#33](https://github.com/faberline/core/issues/33), [#34](https://github.com/faberline/core/issues/34), [#35](https://github.com/faberline/core/issues/35), [#36](https://github.com/faberline/core/issues/36) | Map original cross-language acceptance to executed TypeScript/Python/Rust toolchains and contracts; missing toolchains, skipped languages and unmapped legacy document gates stay blocked. |
| [#42](https://github.com/faberline/core/issues/42), [#45](https://github.com/faberline/core/issues/45) | Verify all role/status and downstream keep contracts, including follower/learner acceptance; do not infer whole-issue completion from the leader-side summary. |
| [#9](https://github.com/faberline/core/issues/9) | Distinguish the shared helper from the original adoption/parity rollout; CLI and served-schema parity belong to authorized online functional gates. |

These are follow-ups on existing contracts, not new feature issues or milestone
expansion. Public issue/PR metadata shows no #73 owner or linked PR at snapshot
time; the local lease check was empty. External owner activity remains unknown.
This work writes only the assigned isolated unit/static preparation tree and
does not take over PR #51 or another author's worktree.
