# Formal Core pipeline proposal

This extends issue #73 under the human's 2026-10-29 delivery request.
The current default policy remains failure-only until the proposed policy has
complete current-head CI and independent Claude review. This document is not
an approval receipt. It does not mark any product issue complete.

The candidate configuration is `.github/issue-ci-required-gates.json`.
It binds the real Core CI workflow ID and reviewed Git blob. It requires all
144 jobs: the original five crate jobs, three exact issue 47 gates, inventory and format, 70 complete
unit/static rows, 62 integration rows, one measured-performance job and one
read-only source-issue inspection job.
Every changed path, including documentation and unknown paths, selects the
whole matrix. There are no category exemptions. The reviewed shared v2 selector
binds each linked issue's exact title and body.
Unmapped or unresolved issue requirements prevent qualification and paid review.
The candidate inspection job reads PR policy as data and does not approve it.
The three separate issue #47 jobs retain its exact proposal-filter test, the
combined Raft test command and Raft runtime all-target Clippy. The proposal job
lists the selected tests, requires a nonzero roster and checks that every listed
name passes. A filtered count for unrelated tests is expected only in this
explicit source gate. Other complete-suite rosters still reject filtered tests.
These jobs do not by themselves prove issue #47's negative control, downstream
behavior or other source-specific obligations; those remain held until mapped.
The existing shared selector rejects incomplete files, unknown rename sources
and mismatched head/base workflow definitions. Workflow, runner, policy and
Cargo manifest changes
require a separately reviewed policy update. A PR cannot approve its own gates.

Every unit/static row also builds its package documentation and runs the
complete profile's doctests. This includes issue #16's explicit all-features
documentation gate. Any ignored examples require a complete release run with
`--include-ignored`; they cannot disappear from its passing roster. A real empty
doctest collection is recorded as no examples, not as unit evidence. Missing
harness output fails, and the separate mandatory unit target remains nonzero.

The 62 integration rows derive from all 29 current Cargo integration targets
and every applicable default, no-default, individual-feature and all-features
profile. Each enabled target runs in release mode with `--include-ignored`.
A complete nonempty named roster is required. Failed, ignored, filtered,
missing, duplicate and partial target results fail. Failure in one target does
not suppress later target evidence. Unit-only packages remain in all 70 unit
rows; the absence of a declared integration target is not a functional waiver.

Live service-k8s certificate and RBAC tests use a disposable Kind cluster on
the hosted runner. The workflow requires its exact Kind context and deletes
that cluster afterward. It does not use an existing local or production cluster.
Legacy EC probes receive the actual workspace root marker. Their nested product
commands still execute; the marker supplies no fake test result.

The measured-performance job runs complete release targets, not filtered
smokes. The existing HTTP/2 workload remains 128 sequential requests in less
than 5 seconds. The snapshot allocator workload remains 1,000 versus 50,000
entries, with the large peak below four times the larger of the small peak and
64 KiB. The test emits its actual allocation measurements without changing
any assertion. The runner checks both named test execution and actual values.
Missing or duplicated measurements fail. Raw logs, command/profile, head/base,
run/attempt and result SHA-256 hashes identify the evidence.

These are repository baseline gates. Each source issue retains its complete
acceptance and additional consumer, cluster, language, oracle or performance
requirements. In particular Core #15's Tape adapter gate and #45's Keep
follower/learner integration are not proven by Core-only results. A passing
baseline is only a prerequisite for the final independent Claude review.
Missing issue-specific evidence must block that review and return the issue to
`todo`; it cannot be converted into an inferred waiver or automatic closure.
The 2026-10-29 inventory records all 23 then-open PRs, their changed paths,
source issue hashes and observed head/base identities. Each changed path selects
the full workspace, including all reverse dependents. This snapshot and the
earlier per-PR inventories are historical observations, not acceptance evidence.
Recheck current head, base, issue and file list before qualification.

After reviewed adoption, qualified CI plus an authenticated current-head
Claude PASS can queue GitHub auto-merge and close the one linked issue.
Review fail/block returns the linked open issue to `todo`, preserves unrelated
labels, and never merges it. The candidate producer/writer pins the reviewed
immutable shared controller at AW `8e2e93e` (PR #112), with 438
complete controls and an independent Claude PASS. It still requires reviewed adoption on
Core before it can operate there.

## First online diagnostics

The first exact-head run at `5d0f500b148ad335e127646b0c26089827a892f6`
exposed host-specific Python cache paths, an unprepared offline generated-Rust
client dependency registry, and a pre-existing obsolete legacy runtime golden
fingerprint. The repairs use the test's own temporary directory, prepare the
same reqwest/serde dependency contract before offline compilation, and pin the
legacy fingerprint to the unchanged current-main emitted runtime bytes. Other
golden entries and all runtime assertions remain intact.

The first allocator receipt measured zero *additional* allocations at both
workload sizes. Zero is a valid numeric measurement under the original bound;
missing measurement records remain failures. The parser now distinguishes
those cases. HTTP/2 actually measured 0.030828 seconds for 128 requests in that
historical run; this is not evidence for a later head.

The first full Raft integration target passed all 170 named cases, but a test's
stdout interleaved with libtest's status line under `--nocapture`. Integration
runs now retain the ordinary captured harness output so the complete named
roster can be checked strictly. They still execute all targets and ignored cases.
The separate measurement job retains uncaptured measured values. No parser
accepts partial rosters, and no product assertion or test count is waived.

The `f3491c0` full run failed one Raft integration test before its handoff
measurement: the initial three-voter cluster did not elect a leader within the
existing setup wait. Its other 139 jobs passed. Four complete unfiltered
integration-target diagnostic runs passed on fresh hosted runners: two with
default scheduling and two with two test threads, each with 170 passing tests.
Those runs do not establish the cause or qualify a later head. The test helper
now records each node's actual `/raftz` state after an election timeout. It does
not change the election wait, handoff budget, concurrency or assertions.
