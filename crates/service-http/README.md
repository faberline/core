# service-http

## Brief

`service-http` provides reusable HTTP service policy. It supplies standard
operational routes, a JSON error envelope, request limits, admission responses,
request trace context, `Server-Timing`, and lifecycle adapters.

The crate does not own a listener or an application's domain routes. It
composes `server-http`, `server-lifecycle`, and `service-observability` where
their mechanisms are needed.

## Primary workflow

1. Create the service router and its domain routes in the app.
2. Mount the standard operational routes.
3. Add the body-limit, admission, trace, and timing layers that the service
   needs.
4. Map domain failures into `ApiErr` and `ErrorEnvelope`.
5. Serve the router through the lifecycle adapter and a caller-owned lifecycle.

## Compose HTTP policy

The public mechanisms are separate. A service can adopt only the parts it
needs.

| Mechanism | Current behavior |
|---|---|
| Standard routes | Mounts `GET /healthz`, `/readyz`, `/metrics`, `/openapi.json`, and `/docs`. |
| Error envelope | Renders `{"error","message"}` JSON with a caller-selected status. |
| Body limit | Rejects an oversized or overrun request body with a structured `413`. |
| Admission | Applies caller-defined token buckets and returns a structured `429` with `Retry-After`. |
| Request trace | Accepts a valid W3C `traceparent` or creates local trace and span IDs for the request span. |
| Server timing | Adds total request duration and exposes opt-in named phases through `ServerTimingExt`. |
| Lifecycle adapters | Connects probe readiness, signals, listener serving, and one shutdown report to a shared lifecycle. |

The admission caller owns route classification, policy values, and the opaque
key. The service owns domain error meaning. The service also decides when it is
safe to disclose named timing phases.

## Use lifecycle composition

Create one `LifecycleController`. Pass it to `lifecycle_probe_routes` and
`serve_with_lifecycle`. Use `LifecycleShutdownTrigger` with
`run_signal_bridge` or `shutdown_on_signal` so readiness, admission, drain, and
the shutdown deadline observe the same lifecycle generation.

`standard_probe_routes`, `ReadinessHook`, `serve`, and `shutdown_with_drain`
remain compatibility adapters. New production composition should use the
lifecycle-aware forms.

## Contract discovery

| Need | Source of truth |
|---|---|
| Public Rust API | `cargo doc -p service-http --no-deps` |
| Standard operational routes | `crates/service-http/src/interfaces/probes.rs` |
| Error and request protection | `crates/service-http/src/interfaces/error.rs`, `interfaces/body_limit.rs`, `interfaces/admission_middleware.rs`, and `application/admission.rs` |
| Request trace and access logging | `crates/service-http/src/interfaces/transport/trace_context.rs` and `access_log.rs` |
| Server timing | `crates/service-http/src/interfaces/server_timing.rs` |
| Lifecycle adapters | `crates/service-http/src/interfaces/transport/serve.rs` and `interfaces/signal.rs` |
| Executable behavior | `cargo test -p service-http` |

## Capabilities

Every entry below is an equal library capability. Each source states its direct
contribution.

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Standard operational routes | `standard-operational-routes` | Mount the common health, readiness, metrics, OpenAPI, and docs routes on an app router. | `crates/service-http`, `crates/service-observability` |
| Structured HTTP errors | `structured-http-errors` | Render generic HTTP failures through one `{error,message}` JSON envelope. | `crates/service-http` |
| Request protection | `request-protection` | Enforce a streaming body limit and caller-defined admission buckets with standard rejection responses. | `crates/service-http` |
| Request observability | `request-observability` | Correlate inbound request traces and disclose bounded server timing. | `crates/service-http`, `crates/service-observability` |
| Lifecycle HTTP adapters | `lifecycle-http-adapters` | Connect HTTP serving, probes, signals, and shutdown reporting to one lifecycle. | `crates/service-http`, `crates/server-http`, `crates/server-lifecycle` |

### Standard operational routes

- ID: `standard-operational-routes`
- Promise: Mount the common probe, metrics, OpenAPI, and interactive docs
  routes from caller-supplied readiness, metrics, and API inputs.
- Sources:
  - [`crates/service-http`](./) provides the Axum route set and canonical JSON
    variant.
  - [`crates/service-observability`](../service-observability/) provides the
    metric-provider contract re-exported by this crate.
- Gate: `cargo test -p service-http`

### Structured HTTP errors

- ID: `structured-http-errors`
- Promise: Pair a caller-selected HTTP status with the shared
  `{error,message}` response shape.
- Sources:
  - [`crates/service-http`](./) provides `ErrorEnvelope`, `ApiErr`, JSON rendering,
    and the reusable OpenAPI schema type.
- Gate: `cargo test -p service-http`

### Request protection

- ID: `request-protection`
- Promise: Reject oversized request bodies with `413` and denied admission
  attempts with `429` plus `Retry-After` without retaining a raw caller key.
- Sources:
  - [`crates/service-http`](./) provides the streaming body layer, admission
    controller, redacted observation, and shared response envelopes.
- Gate: `cargo test -p service-http`

### Request observability

- ID: `request-observability`
- Promise: Accept or create an inbound trace context, correlate the request
  span, and attach a bounded `Server-Timing` response value.
- Sources:
  - [`crates/service-http`](./) provides W3C header parsing, request span fields,
    access-log adapters, and server timing middleware.
  - [`crates/service-observability`](../service-observability/) provides the
    tracing and optional OTLP mechanisms used by the adapters.
- Gate: `cargo test -p service-http`
- Gate: `cargo test -p service-http --features otlp --test it -- otlp_tracing::`

### Lifecycle HTTP adapters

- ID: `lifecycle-http-adapters`
- Promise: Use one lifecycle for readiness probes, signal handling, listener
  drain, and the terminal shutdown report.
- Sources:
  - [`crates/service-http`](./) provides the router and signal composition
    adapters.
  - [`crates/server-http`](../server-http/) owns the HTTP listener and its drain.
  - [`crates/server-lifecycle`](../server-lifecycle/) owns lifecycle state and
    terminal reporting.
- Gate: `cargo test -p service-http`

## Supporting documents

| Document | Use it for |
|---|---|
| [STATUS.md](STATUS.md) | Current HTTP policy support and limits |
| [ROADMAP.md](ROADMAP.md) | Future shared outcomes and non-goals |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Edit rules and required verification |
