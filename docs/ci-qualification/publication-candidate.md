# PR execution scope

Refs [#73](https://github.com/faberline/core/issues/73) and
[draft PR #74](https://github.com/faberline/core/pull/74).

The human authorized publication, online testing and remaining CI repairs.
The branch is `codex/73-pr-unit-static-preparation`, based on
`41c352eaae687a1914087d279f36adbe9e9b4af5`.
The isolated worktree is `/tmp/core-73-pr-gates.li3y5I/core`.
The original prepared WIP remains unchanged at
`/Users/chrischeng/faberlines/core-73-unit-static`.

## What runs

The original five crate jobs remain unchanged. New PR-only jobs derive a fresh
workspace inventory, check formatting independently and run the complete
package/feature unit and all-target Clippy matrix. The current inventory has
30 library targets and 70 package/profile rows. At most four rows run at once.
Each matrix job has a 30-minute timeout. No row is dropped to get a green run.
The planner refuses over 256 rows rather than truncating the matrix.

The base and candidate are checked out at the event's exact SHAs. Candidate
helper code runs unprivileged, with `contents: read`, no added secrets and no
persisted checkout credentials. The base supplies the comparison inventory.
It need not already contain the new helper. That requirement blocked all new
ordinary tests in run 40643971191 and has been removed from PR execution.

Zero, ignored, measured, filtered, incomplete or duplicate test evidence still
fails. Each enabled unit target runs even if an earlier target fails. Clippy
runs after failed unit tests unless the workflow is cancelled. Formatting
changes in Rust sources were generated only by `cargo fmt --all`.

## What the results mean

These are candidate checks. They do not install trusted qualification, clear
failure-only blockers or close an issue. Formal adoption still needs approved
immutable workflow/driver/evaluator identities, current-head required evidence,
issue-specific functional/E2E and measured-performance mappings, independent
review and authorized delivery. The old independent review is historical; it
does not apply to the revised workflow or planner.

Local verification passed 45 helper controls and 10 existing controller
controls. Fresh offline metadata yields the same 70 rows. Workspace formatting
now passes. No product build or product test ran locally. Online results must
be checked on the final PR head. Existing push/tag/manual behavior, policy,
controller, permissions and infrastructure remain unchanged.
