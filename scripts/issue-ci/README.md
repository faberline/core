# Core CI and Claude outcome controller

The production entrypoint is the one `.github/workflows/issue-ci.yml` writer.
It calls the reviewed immutable AW native-auto-merge action. It reads GitHub
metadata and authenticated review logs; it does not checkout or execute PR code
on its credentialed runner. The separate read-only test jobs execute candidates.

The only operational policy is `.github/issue-ci-required-gates.json`.
It declares complete workspace checks, exact workflow identities and protected
paths. Qualification must compute the actual changed paths, current head/base
and complete required jobs. A raw green job list cannot approve a PR.

`handoff.py`, `qualification.py` and `json_data.py` are exact copies from the pinned AW source
for isolated controls and inspection. They must not drift from that source.
`check-policy.py` validates the policy without a lifecycle mutation:

```sh
python3 scripts/issue-ci/check-policy.py .github/issue-ci-required-gates.json
```

Product suites run through the authorized online PR workflow. Local verification
is limited to isolated controls and static/metadata inspection. The historical
failure-only policy fixture retains its old negative controls; it is not the
operational configuration.

Qualified CI is a prerequisite for independent Claude review. A passing current
review enables guarded native auto-merge and linked issue closure. A verified
fail/block returns its managed issue to `todo`. Stale or unknown evidence cannot
approve a merge. Policy/workflow changes need separately reviewed adoption;
they cannot use their own proposed policy to approve themselves.
