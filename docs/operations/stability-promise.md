# Stability promise

From 1.0.0, core follows semantic versioning. No breaking change is made before 2.0.0.

## Rule

- A breaking change is allowed only in a major version. A breaking change is a
  removed or changed public item in the "Stable public surface" column, a
  changed wire format, a changed on-disk format, or a changed meaning of a CRD field.
- Additions are minor. Fixes are patch.
- A breaking change lists itself in `docs/migration/` (see "Releasing" in
  `docs/operations/README.md`).
- A crate whose evidence is "not measured" has the promise but no proof yet.

## Crates

| Crate | Stable public surface | Wire format | On-disk format | CRD | Evidence |
|---|---|---|---|---|---|
| `build-stamp` | `build_stamp::stamp` | none | none | none | docs/domain/build-stamp.md |
| `claim-token` | `claim_token::{sign, verify, ExpiryUnixSeconds, InputKey, ResultKey, Scope}` | `b64url(json(scope)).hex(hmac-sha256)` token | none | none | docs/domain/claim-token.md |
| `cli-std` | `cli_std::api::{artifact, chainable, connect, issue, llm, registry, report_issue, upgrade}` | JSON output | none | none | docs/domain/cli-std.md |
| `compass` | `compass::api::{check_pipeline, core, diagnostic, gen, graph, lens_error, lint, lsp, output, schemas, semantic, server}` | JSON-RPC 2.0 | none | none | docs/domain/compass.md |
| `e2e-support` | `e2e_support::{domain, infrastructure}` | none | none | none | not measured |
| `index-text` | `index_text::{for_jieba_no_hmm, JiebaRouteStore}` | JSON snapshot (`TextIndexSnapshot`) | versioned JSON snapshot, format version 1 | none | docs/domain/index-text.md |
| `metrics-prometheus` | `metrics_prometheus::{Bucket, Counter, Gauge, Histogram, Latency, render}` | Prometheus text format 0.0.4 | none | none | docs/domain/metrics-prometheus.md |
| `metrics-remote-write` | `metrics_remote_write::{proto, consume_write, RemoteWriteConsumer, validate_headers}` | Prometheus Remote Write 1.0 (protobuf, Snappy) | none | none | docs/domain/metrics-remote-write.md |
| `openapi-codegen` | `openapi_codegen::api::{emit, ir, llm}` | OpenAPI 3.0, 3.1, 3.2 input | `.openapi-codegen.json` manifest | none | crates/openapi-codegen/STATUS.md |
| `peer-tls` | `peer_tls::{material, reload, PeerTlsConfig, install_default_crypto_provider}` | mutual TLS, HTTP/2 | none | none | docs/domain/peer-tls.md |
| `raft-core` | `raft_core::domain` | none | none | none | docs/domain/raft-core.md |
| `raft-runtime` | `raft_runtime::api::{conformance, llm}` | peer HTTP/2 bodies | none | none | docs/domain/raft-runtime.md |
| `server-http` | `server_http::{tls, H2cServerOptions, HttpServerOptions, serve_h2c, trace_layer}` | HTTP/1.1 and h2c | none | none | crates/server-http/STATUS.md |
| `server-lifecycle` | `server_lifecycle::{config, deadline, drain, hooks, lifecycle, limits, metrics, readiness, signal, task_supervisor}` | none | none | none | docs/domain/server-lifecycle.md |
| `server-tcp` | `server_tcp::{TcpServerConfig, TcpSocketOptions, TcpHandler, serve, bind}` | none | none | none | docs/domain/server-tcp.md |
| `service-auth` | `service_auth::api::{gcp, k8s, llm, reload, static_token}` | HTTP bearer token | none | none | crates/service-auth/STATUS.md |
| `service-backup` | `service_backup::api::llm` | JSON backup object | `<prefix>-<unix seconds>.json` (local, GCS, S3) | none | docs/domain/service-backup.md |
| `service-collector` | `service_collector::{application, domain, infrastructure}` | JSONL quarantine | JSONL quarantine file, JSON checkpoint | none | docs/domain/service-collector.md |
| `service-executor` | `service_executor::{bounded, group_commit, job_runner}` | none | none | none | docs/domain/service-executor.md |
| `service-http` | `service_http::api::transport` | JSON error envelope `{"error","message"}` | none | none | crates/service-http/STATUS.md |
| `service-k8s` | `service_k8s::api::{certificate, controller, crd, lease, lifecycle, llm, metrics, render, resize, rollout, service, stateful}` | none | none | `ManagedService` (namespaced CR) | crates/service-k8s/STATUS.md |
| `service-mcp` | `service_mcp::{McpApplication, HttpTransportConfig, streamable_http_router, serve_stdio}` | Model Context Protocol (streamable HTTP, stdio) | none | none | docs/domain/service-mcp.md |
| `service-observability` | `service_observability::{config, filesystem, jsonl, logging, metrics, process}` | JSON log line (`axiom.service.log.v1`) | none | none | docs/domain/service-observability.md |
| `service-projection` | `service_projection::{application, domain, infrastructure}` | JSON state, three published OpenAPI schemas | `indexes/<name>/state.json`, format version 1 | none | docs/domain/service-projection.md |
| `storage-durable` | `storage_durable::{atomic, capacity, data_root, framed_log, fsync, generation, snapshot_store}` | none | framed log and snapshot files | none | docs/domain/storage-durable.md |
| `storage-object` | `storage_object::{error, object, store, local, gcs, s3, scratch}` | none | object store (local, GCS, S3) | none | docs/domain/storage-object.md |
| `storage-segment` | `storage_segment::{application, domain}` | JSON catalog page | `<prefix>/pages/<sha256>.json` pages | none | docs/domain/storage-segment.md |
| `surface` | `surface::{Callback, Component, Element, Props, SurfaceSnapshot, SurfaceNode}` | serde JSON snapshot | none | none | docs/domain/surface.md |
| `transport-h2c` | `transport_h2c::{h2c_client, H2cManager, H2cPool, server}` | HTTP/2 cleartext (h2c, prior knowledge) | none | none | crates/transport-h2c/STATUS.md |
| `transport-otlp` | `transport_otlp::{proto, codec, consumer, grpc, media, payload, signal}` | OTLP protobuf over HTTP and gRPC | none | none | docs/domain/transport-otlp.md |
| `ui-runtime` | `ui_runtime::domain` | none | none | none | docs/domain/ui-runtime.md |
