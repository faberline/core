# Formal Core pipeline proposal

This extends issue #73 under the human's 2026-10-29 delivery request.
The current default policy remains failure-only until the proposed policy has
complete current-head CI and independent Claude review. This document is not
an approval receipt. It does not mark any product issue complete.

The proposed configuration is `.github/core-coverage-policy.json`.
It binds the real Core CI workflow ID and reviewed Git blob. It requires all
140 jobs: the original five crate jobs, inventory and format, 70 complete
unit/static rows, 62 integration rows and one measured-performance job.
Every changed path, including documentation and unknown paths, selects the
whole matrix. There are no category exemptions. The existing shared selector
rejects incomplete files, unknown rename sources and mismatched head/base
workflow definitions. Workflow, runner, policy and Cargo manifest changes
require a separately reviewed policy update. A PR cannot approve its own gates.

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
The existing 22 product PR inventories are historical and must be refreshed
at their own current heads before using their issue evidence.

After reviewed adoption, qualified CI plus an authenticated current-head
Claude PASS can queue GitHub auto-merge and close the one linked issue.
Review fail/block returns the linked open issue to `todo`, preserves unrelated
labels, and never merges it. The shared deployed rejection code still needs
the human's latest doing-to-todo correction before this rollout is complete.
