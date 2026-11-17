# e2e-support

## Brief

e2e-support is the shared end-to-end test harness for Faberline service
repositories. It is a dev-dependency of those repositories and is not published
(`publish = false`). It holds two layers: `domain` has pure checks (identity,
pool placement, evidence lines, names, inspect helpers) and `infrastructure` has
the helpers that run them (kubectl, waits, HTTP calls, the namespace variable).

## Contributing

Project-local authoring and verification rules live in [CONTRIBUTING.md](CONTRIBUTING.md).

## Public API

- `required`, `namespace`: read a required variable, or the run namespace (`E2E_NAMESPACE`, else `NAMESPACE`).
- `say`, `evidence`, `Step`: write a progress line, or an `E2E_EVIDENCE` line, and time a step.
- `Kube`, `DumpOnFailure`: run `kubectl` in the namespace, and dump diagnostics on failure.
- `wait_for`, `wait_pods_exist`, `wait_ready`: poll until a probe passes or a timeout ends.
- `Api`: an HTTP client for the service under test.
- `Identity`: check the image and version identity of a build.
- `PoolPlacement`: check that a pod or workload runs on the expected node pool.
- `normalize_runtime_image_id`, `is_dns_label`, `is_lower_hex`, `named`, `env_count`, `mounts_named`: pure checks.
- `evidence_line`, `EVIDENCE_PREFIX`: the exact shape of an evidence line.

## Capabilities

None claimed yet. A capability belongs here with the verbatim gate command that
verifies it; a promise with no gate under it is not claimed.
