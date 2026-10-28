# Local verification of the unit/static preparation

Refs [#73](https://github.com/faberline/core/issues/73). Verified at
2026-10-27 09:52 UTC in branch `codex/73-unit-static`, based on
`41c352eaae687a1914087d279f36adbe9e9b4af5`.

These are isolated helper/static results. No product build, Cargo test,
clippy, fmt, integration/e2e, performance, online workflow or paid review was
executed. No issue, lease, qualification policy or lifecycle state was changed.

## Measured results

Python below is `/Users/chrischeng/faberlines/aw/.venv/bin/python` (3.13.9).

| Check | Result |
|---|---|
| `python -m unittest discover -s scripts/pr-gates/tests -q` | 43 passed, 0 failed; 0.014 seconds, after independent-review fixes |
| `python -m unittest discover -s scripts/issue-ci/tests -q` | Existing controller fixtures: 10 passed, 0 failed; 0.005 seconds |
| `cargo metadata --locked --no-deps --offline --format-version 1` | Metadata only: 30 libraries, 0 binaries, 29 integration targets, 3 examples, 10 feature-bearing crates |
| `python scripts/pr-gates/plan.py matrix --metadata-json /tmp/core73-workspace-metadata.json` | 70 rows across all 30 packages: 30 default, 10 no-default, 20 individual feature, 10 all-features |
| Ruby standard-library YAML parse and comparison with base workflow | Parses successfully; original five-crate job, triggers, permissions and concurrency unchanged |
| Python syntax compilation in memory and JSON parsing | Passed |
| `bash -n` over the eight workflow run steps | Passed |
| Actual bootstrap shell control with current baseline checkout | Expected exit 1: trusted base has no driver; no fallback or matrix execution |
| Policy/controller byte comparison with base commit | `.github/issue-ci-required-gates.json`, `scripts/issue-ci/handoff.py`, `scripts/issue-ci/check-policy.py` unchanged |
| `git diff --check` | Passed |

`actionlint` is not installed in this environment. The YAML parse is a syntax
and preservation check, not a claim that GitHub accepted or executed the new
workflow. Product execution and all-target clippy remain unmeasured online
obligations. The matrix is not a coverage percentage or a qualification result.

## Initial selection negative control

In a temporary copy of `scripts/pr-gates/`, replace exactly this assignment:

```python
expanded = changed | {p['name'] for p in packages if changed & set(p['dependencies'])}
```

with `expanded = changed`. From that copy's `tests/` directory run:

```sh
python -m unittest test_plan.SelectionTests.test_reverse_dependencies_are_transitive -v
```

Observed exit code 1 and the following failure:

```text
FAIL: test_reverse_dependencies_are_transitive (test_plan.SelectionTests.test_reverse_dependencies_are_transitive)
AssertionError: Lists differ: ['a', 'b', 'c'] != ['a']
- ['a', 'b', 'c']
+ ['a']
Ran 1 test in 0.000s
FAILED (failures=1)
```

This control was recorded against the earlier candidate. Its `plan.py` SHA-256
before and after that temporary-copy mutation was identical:
`46199b087459564bfef8dcb941afff2fb96aafa048cc3ec3cf5e212581dbc0fb`.

## Final roster negative control

After the independent review fixes, a temporary copy changed only
`if not counts['passed']:` to `if False:`. Running
`python -m unittest test_evidence.LibtestTests.test_zero_test_target_requires_reviewed_applicability -v`
from that copy's tests directory produced:

```text
FAIL: test_zero_test_target_requires_reviewed_applicability (test_evidence.LibtestTests.test_zero_test_target_requires_reviewed_applicability)
AssertionError: ValueError not raised
Ran 1 test in 0.000s
FAILED (failures=1)
```

Exit code was 1. The final repository source was not mutated; its SHA-256 before
and after was identical:
`4afb5fc0a833d13460fb50ddc9efb52e27d40c05ec381303145059ba4c4473ce`.
The unchanged final source passes all 43 helper tests. New fixtures cover actual
manifest `harness=false` even when metadata says `test=true`, event HEAD/base
identity, baseline shrink before execution, per-target roster association,
missing/partial/duplicate/zero/filtered/ignored/measured evidence and preserved
stdout/stderr on process failure. The mock results are not product execution.

## Remaining evidence

The branch has not been published or dispatched. The 22 PR snapshots are
historical reads at their recorded HEADs, not newly executed test evidence.
Keep existing failure-only qualification blockers until approved mappings and
complete current-head online evidence exist. Final Claude review and separately
authorized delivery remain outstanding. This local report does not close #73
or alter any source issue's acceptance.
The current baseline has no driver, so the proposed new checks fail closed
pending independently reviewed bootstrap. Ordinary PR-owned workflow success
still requires externally approved immutable workflow/driver identity before
it can become trusted qualification evidence.

## Publication refresh, 2026-10-28

The current controller copied the complete prepared diff into an isolated
`codex/73-pr-unit-static-preparation` worktree, preserving the original WIP.
It reran 43 helper and 10 existing controller fixtures (53 total, all passed),
read fresh offline Cargo metadata, and reconfirmed all 70 matrix rows.
Driver/workflow hashes still match the historical independent review. The
trusted policy/controller bytes remain equal to the base. YAML parsing and
diff whitespace checks passed. These are controller rechecks, not a new
independent review, online product evidence or bootstrap approval.

The subsequent controller change separates workspace format from inventory, so
format failure cannot skip all 70 unit rows. Original five-crate jobs remain
unchanged. The driver hash remains the reviewed value; the workflow hash changed
and requires fresh independent review. Historical workflow review is not reused
for this modification.
