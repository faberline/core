# Formal Claude review and PR completion

The candidate pins shared source
`faberline/aw@568c7512c5d4daad65e0430b8b6e623ad8ee4835` (AW #94).
Its 354 complete shared/security controls and independent Claude review passed.
This is not yet a Core adoption receipt.

The candidate's one authoritative policy is
`.github/issue-ci-required-gates.json`. It requires the complete 140-job
workspace baseline described in [formal-pipeline.md](formal-pipeline.md).
Every declared unit/static and integration feature row is required. Missing,
skipped, cancelled, stale or partial evidence blocks qualification. There are
no category exemptions or documentation/unknown-path zero-check passes.

The formal read-only Claude producer runs only after trusted qualification.
A current-head PASS plus every required gate enables native squash auto-merge.
GitHub closes the one `Closes #N` issue after merging to the default branch.
A valid fail/block returns the linked managed issue to `todo`, preserving
unrelated labels. Receipt-bound recovery cannot undo a later retry claim.

Workflow, driver, Cargo manifest and policy changes require separately reviewed
adoption. A candidate cannot approve its own protected gate changes. The old
default policy remains failure-only until the exact candidate has complete
online CI, independent review and verified delivery. Old product heads with an
untrusted workflow/base or missing issue-specific evidence remain held.

The CI handoff and native merge writer use identical complete qualification
semantics. Existing historical failure receipts are not qualifying evidence.
Full CI still does not substitute for each source issue's additional acceptance
or for the final independent review.

Formal workflow ID: 399789974.
Unified writer ID: 394215097.
Native protection ruleset: 26069017.
Required status: `claude/verified-merge` by GitHub Actions app 15368.
