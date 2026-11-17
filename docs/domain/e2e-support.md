# e2e-support

e2e-support is the shared end-to-end test harness for service repositories. It
gives the test suites of those repositories pure checks (identity, pool
placement, evidence lines, names) and the helpers that run them: kubectl, waits,
HTTP calls and the run namespace. It owns no service behaviour and is not
published. The design is in [shared test tooling](../operations/shared-test-tooling.md).

**Form:** domain and infrastructure layers · **Depends on:** — · **Crate:** [`crates/e2e-support`](../../crates/e2e-support)

## Model

- **Layers** — `domain` holds the pure checks with no I/O. `infrastructure`
  holds the kubectl, wait, HTTP and environment helpers.
- **Identity** — `Identity`: the image, digests, version and git sha of a build.
  Its version must equal the version the caller expects.
- **Pool placement** — `PoolPlacement`: a node selector and an image pull
  secret that a workload must carry.
- **Evidence line** — `evidence_line`: one line, `E2E_EVIDENCE {"suite","step","data"}`.
- **Run namespace** — `namespace`: `E2E_NAMESPACE`, with `NAMESPACE` as the fallback.

## Ports

- `Api` — the HTTP client to the service under test. It is the only port to a
  running service.
- `Kube` — the kubectl runner for the run namespace.

## Invariants

- The evidence line has the exact shape `{"suite","step","data"}`, on one line,
  with no `ok` key.
- `Api::call` retries only a transport failure, up to the retry budget. It
  panics on a 4xx or 5xx status at once and does not retry it.
- `Api::answers` never panics and never retries.
- `Identity`'s version must equal the caller's argument; otherwise
  `version_error` names the mismatch.
- `required` and `namespace` panic when the variable is missing or empty. They
  never skip.

## Published language

The whole public API of the crate, listed in its [README](../../crates/e2e-support/README.md).

## Exceptions and debts

None.
