# service-http

service-http is the HTTP service kit that every faberline service composes: the
standard probe endpoints, HTTP admission, request tracing and access logs, the
JSON error envelope, request-body limits and decoding, `Server-Timing`, signal
handling and a policy-driven reverse proxy. It composes and does not replace:
server-http owns the listener, service-observability owns logging and metric
semantics, and a service mounts its own routes on the routers this crate
returns. Authentication and backup are out of scope. No core crate depends on
it; downstream, beam, courier, defer, keep, loom, lumen, pgpool, relay, sift,
tape and workspace do.

**Form:** layered · **Depends on:** server-http, server-lifecycle, service-observability · **Crate:** [`crates/service-http`](../../crates/service-http)

## Model

- **HTTP admission** — token-bucket admission of requests per class and key.
  `AdmissionPolicy` (capacity, refill window, key cap) per class, held by an
  `AdmissionController`; `AdmissionConfig` reads the policies from
  `<SVC>_ADMISSION_*` variables. Each request is an `AdmissionInput`, and the
  result is an `AdmissionDecision` with an `AdmissionOutcome`
  (`Bypass`, `Allow`, `Deny`).
- **Weighted admission** — `WeightedAdmission`: per-key concurrency plus a
  weight quota per window, under `WeightedAdmissionConfig` (checked `new`,
  getters).
- **Concurrency lease** — `ConcurrencyLease`: a held slot of weighted
  admission, released on drop.
- **Reverse proxy policy** — `ReverseProxyPolicy`: which upstream a request
  goes to, the body cap and the upstream timeout.
- **Error envelope** — `ErrorEnvelope` (`error`, `message`) and
  `DetailedErrorEnvelope` (adds `retryable`, projection cursors and
  `retry_after_seconds`), rendered through `ApiErr`. `ProjectionMetadata`
  names the projection and its required and current cursors.
- **Probe routes** — `standard_probe_routes` and `lifecycle_probe_routes`:
  `/healthz`, `/readyz`, `/metrics`, `/openapi.json` and `/docs`.
- **Request trace context** — `RequestTraceContext`: the W3C trace context a
  request carries or is given, used by the service-http trace layer
  (`trace_layer`, target `http.access`) and its span makers.
- **Content decode limits** — `ContentDecodeLimits`: compressed and decoded
  byte caps for `decode_request_body` (checked `new`, getters).
- **Server timing** — `ServerTimingExt` entries and a `ServerTimingDisclosure`
  (`TotalOnly` by default, `Full` per response).
- **HTTP config** — `HttpConfig`: bind address, log settings, grace period,
  body limit and OTLP endpoint. `new` takes every value; getters read them.
- **Shutdown trigger** — `LifecycleShutdownTrigger`: turns a signal into a
  server-lifecycle shutdown with a validated total and reserve.

## Ports

- `ReverseProxyPolicy` — upstream selection for the proxy; sift implements it.
- `AdmissionObserver` — receives each `AdmissionEvent`;
  `NoopAdmissionObserver` is the default.
- `ReadinessHook` — server-lifecycle's `Readiness` under its service-http name;
  courier, defer, keep, loom, lumen, relay, sift and tape implement it.
- `MetricsProvider` — service-observability's trait, re-exported; defer, keep,
  loom, lumen, relay, sift and tape implement it.

## Invariants

- An admission key is hashed with SHA-256 before it reaches bucket state, and
  `AdmissionInput` is neither `Debug` nor `Serialize`. Observers see only the
  class, the outcome and the retry delay.
- A policy with a zero capacity, refill window or key cap is rejected. A
  request of a class with no policy is `Bypass`. At the key cap, the
  least-recently-seen bucket is evicted.
- A denied request gets 429 `rate_limited` with `Retry-After` in whole seconds,
  at least 1.
- The reverse proxy only forwards to an `http` or `https` URL with a host
  (else 502 `invalid_upstream`), strips hop-by-hop headers, `Host` and
  `Content-Length`, and caps both bodies: an oversized request is 413, and an
  oversized response or an upstream failure is 502.
- `ApiErr` renders the two-field envelope unless a detailed field is set;
  setting a retry delay marks the error retryable and sets `Retry-After`.
  `retry_delay_from_detailed_error` trusts only a retryable 429 or 5xx whose
  header and body agree.
- Probe routes carry no auth and no body limit. `/readyz` answers 503
  `draining` while draining. Lifecycle probes add `x-lifecycle-phase`,
  `x-lifecycle-generation` and a sanitized `x-lifecycle-reason-code` of at
  most 64 characters.
- A request keeps its `traceparent` only when it is a single, valid W3C
  version-00 header with lowercase hex and non-zero ids; otherwise the request
  starts a fresh local trace.
- `decode_request_body` accepts only identity and gzip, within both limits;
  `body_limit_layer` answers an oversized body with 413 `payload_too_large`.

## Published language

Downstream crates import from the crate root only: `ApiErr` and the envelopes,
`ProjectionMetadata`, the admission and weighted-admission types,
`ReadinessHook`, `MetricsProvider`, the probe routes, `serve_with_lifecycle`,
`serve_tls`, `trace_layer`, the content-decode types and the signal helpers.
The root also re-exports `HttpServerOptions`, `ServerConfigSource` and
`config_source` from server-http, and `LifecycleMetrics`, `LogFormat`,
`ServiceIdentity` and the tracing initializers from service-observability.

`transport` is the one public module (`src/api/transport.rs`): it also holds
`request_trace_context`, `RequestTraceContext`, `CorrelatingMakeSpan` and the
access-log span hooks, which the root does not re-export. P2 deleted the old
modules `admission`, `body_limit`, `config`, `content_decode`, `error`,
`logging`, `metrics`, `probes`, `readiness`, `reverse_proxy`, `server_timing`,
`signal` and `weighted_admission`: every name in them is at the crate root, and
none had a known external user by path.

## Exceptions and debts

- **Checker exceptions (P1):** None. service-http has no domain layer:
  admission control reads the clock, so it is in the application layer. The
  reverse proxy moved whole into the interfaces layer and uses no
  infrastructure module.
- **Tracked for P2:** clock and id reads: admission and weighted admission read
  `Instant::now` (the `admit_at` and `acquire_at` seams already take the time),
  and fresh trace ids hash the wall clock; P2 adds clock and id-generator
  ports. `ProjectionMetadata` public fields, built with struct literals by
  sift. `anyhow` in `reverse_proxy_router` (ADR D4).
