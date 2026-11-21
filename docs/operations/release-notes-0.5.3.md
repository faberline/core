# Release notes 0.5.3

This page lists the changes from `0.5.2` to `0.5.3`.

## Added

- openapi-codegen: deadline and cancellation in the emitted TypeScript, Python and Rust clients (#608, #609, #611).
- openapi-codegen: retry hooks in the emitted TypeScript and Python clients (#612, #613).
- openapi-codegen: retry hooks in the emitted Rust client (#614).
- openapi-codegen: Rust typed enum for untagged `oneOf` with inline object variants (#607) and internally tagged enum (#610).
- service-http: error details, catalog error codes and request timeout layer (#577); request id middleware (#571); per-operation metrics middleware (#574); opt-in auth and admission defaults (#578); ETag and If-Match check, Idempotency-Key helper, opaque cursor and UUIDv7 maker (#580).
- cli-std: verbs `dockerfile`, `backup`, `spec` and the `k8s_render` rules (#548, #549, #551, #559, #567, #570).
- service-k8s: extra workloads, derived RBAC, status condition per workload (#553, #569, #576).

## Fixed

- compass: rules DK002, DK004, K8003, K8006, RS201, PY305 and SQL-INJ false positives (#596, #604, #605, #606).
- raft-runtime: flaky tests (#557, #558, #581).
- gke-drill: partition drill (#592).
