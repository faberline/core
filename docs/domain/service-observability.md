# service-observability

service-observability owns the shape of a service's telemetry: the log format
and the versioned JSON line a collector reads, the service identity attached
to it, optional OTLP tracing, the `MetricsProvider` seam, the connection and
lifecycle metrics of a server-lifecycle runtime, and process and filesystem
samples. It does not own the transport, the collector, the metric names a
service chooses, or the log-level policy. HTTP adaptation stays in
service-http; Prometheus primitives and encoding stay in metrics-prometheus.
In core, service-http uses it; downstream, sift uses the log schema, and
lumen and defer use it in tests.

**Form:** whole-src infrastructure · **Depends on:** metrics-prometheus, server-lifecycle · **Crate:** [`crates/service-observability`](../../crates/service-observability)

## Model

- **Log format** — `LogFormat`: `Json` is the collector-compatible stdout
  format (`collector_compatible`); `Pretty` is for development only.
- **Service identity** — `ServiceIdentity`: the name and version attached to
  every log line and exported span; neither may be blank.
- **Observability config** — `ObservabilityConfig`: log level, log format and
  an optional OTLP endpoint.
- **Tracing mode** — `TracingMode` (`LoggingOnly`, `Otel`, `OtelUnavailable`
  with an `OtelFallback` of `FeatureDisabled` or `InvalidEndpoint`): what
  `tracing_mode` resolves from the config and the `otlp` feature.
- **Service log event** — `ServiceLogEventV1` (also named
  `StructuredServiceLogV1`) with its `ServiceLogIdentityV1`: one JSON line under
  the schema `SERVICE_LOG_SCHEMA_V1` (`axiom.service.log.v1`), written by
  `ServiceJsonFormatter`. `service_log_schema_v1` returns the checked-in JSON
  Schema.
- **Metrics provider** — `MetricsProvider`: anything that renders a complete
  Prometheus text body; the default renders nothing.
- **Lifecycle metrics** — `LifecycleMetrics`: accepted, rejected and closed
  connection counters plus the lifecycle phase, generation, transition count
  and transition age.
- **Metrics snapshot** — `LifecycleMetricsSnapshot`: the last recorded phase,
  generation, transition count, age and reason code.
- **Process usage** — `ProcessUsage` from `process_usage`: resident memory in
  bytes and cumulative CPU seconds of one process.
- **Filesystem usage** — `FilesystemUsage` from `filesystem_usage`: total, used
  and available bytes of the filesystem that carries a path.

## Ports

- `MetricsProvider` — implemented here by `LifecycleMetrics`, and by services
  through service-http's re-export.
- server-lifecycle's `ConnectionMetrics` is implemented by `LifecycleMetrics`;
  it is not declared here.

## Invariants

- `RUST_LOG` wins over the configured log level. An OTLP endpoint that is not an
  `http` or `https` URI with an authority, a build without the `otlp` feature,
  or a tracer that fails to build falls back to logging only with a warning;
  none of them fails startup.
- Every JSON line carries `SERVICE_LOG_SCHEMA_V1`; changing a field's meaning
  needs a new constant, not an edited one.
- Attributes are bounded: at most 64 (the alphabetically first survive), keys
  cut to 128 bytes and values to 4096 bytes on a UTF-8 boundary. A caller
  attribute that collides with a schema field is dropped, never renamed.
- `authorization`, `proxy_authorization`, `cookie`, `set_cookie`, `baggage` and
  `tracestate` are dropped, not masked: matched case-insensitively with `-` read
  as `_`, and also as a `.`, `/` or `_` suffix.
- A correlation field is written only when valid: `trace_id` is 32 lowercase
  hex digits and `span_id` and `parent_span_id` are 16, none of them all zeros;
  `trace_flags` is 2 hex digits; a request id is non-empty, at most 128 bytes
  and free of control characters.
- `LifecycleMetrics` records each lifecycle generation once: a replayed
  generation changes nothing, and a coalesced jump counts every skipped
  transition. The phase gauge reads 255 until a first observation, and no
  series carries a reason or detail label. `observe_lifecycle` returns at
  `Stopped` or `Fatal`.
- `available_bytes` is the only filesystem figure that answers "can I write
  this": with a reserve, used plus available is less than total. The
  multiplications saturate at `u64::MAX`, and a missing path is an error, not
  a zeroed sample.
- `process_usage` samples through `ps`, so `ps` must be on `PATH` and each
  sample costs a fork; resident memory has 1 KiB resolution.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. The root re-exports the items of `config`, `filesystem`, `jsonl`,
`logging`, `metrics` and `process`, and all six modules stay public.

## Exceptions and debts

- **Checker exceptions (P1):** None.
- **Tracked for P2:** `ServiceLogEventV1` and `ServiceLogIdentityV1` public
  fields, built with struct literals by a sift test fixture. `anyhow` in public
  signatures: `ServiceIdentity::new`, `init_tracing`,
  `init_tracing_with_identity`, `process_usage` and `filesystem_usage` (ADR D4).
