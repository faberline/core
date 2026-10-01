# core

The shared Rust libraries behind the faberline services and CLIs: transport,
server runtime, consensus, storage, auth, observability, Kubernetes integration,
and the CLI standard commands. Split out of
[faberline/axiom](https://github.com/faberline/axiom) `libs/` with full history.

## Crates

| Crate | What it is |
|-------|------------|
| [build-stamp](crates/build-stamp) | Shared build.rs stamping for service CLIs — git short-sha, built-at epoch, and target triple as `cargo:rustc-env=<PREFIX>_*` directives. Consumed as a [build-dependencies] crate. |
| [claim-token](crates/claim-token) | Scoped claim-check access tokens (HMAC-SHA256). Loom's schema layer signs; Keep verifies, so a worker reaches Keep directly only within its key scope. |
| [cli-std](crates/cli-std) | Standard agent-facing CLI commands every axiom tool ships: llm (offline self-docs), upgrade (self-update from GitHub releases), issue (search/view/create diagnostics-rich issues). Parameterized by ToolInfo; clap-agnostic. |
| [compass](crates/compass) | Code intelligence arsenal — navigate, analyze, refactor, and watch any codebase |
| [index-text](crates/index-text) | Rebuildable text-index base for Axiom products. |
| [metrics-prometheus](crates/metrics-prometheus) | Shared lock-free Prometheus counter, gauge, and latency primitives plus the text/plain version 0.0.4 encoder used by service metrics endpoints. |
| [metrics-remote-write](crates/metrics-remote-write) | Prometheus Remote Write 1.0 transport and validation shell. |
| [openapi-codegen](crates/openapi-codegen) | Generate typed TypeScript, Python, and Rust API clients from an OpenAPI 3.0/3.1/3.2 document |
| [peer-tls](crates/peer-tls) | Shared peer-mTLS material loading for mutually authenticated peer and replication ports: PEM loaders, rustls client/server builders, and process crypto-provider setup. |
| [raft-core](crates/raft-core) | Self-contained, step-driven Raft consensus core — transport- and storage-agnostic. Consumers (relay, keep) supply the network, persistence, and state machine. |
| [raft-runtime](crates/raft-runtime) | Shared Raft runtime that drives raft-core for a service state machine over transport-h2c, including apply, snapshot, compaction, topology, and read-your-write proposal handling. |
| [server-http](crates/server-http) | Shared HTTP protocol runtime built on server-lifecycle and server-tcp: HTTP/1.1 plus h2c serving, request tracing, and hooks for service shells and development servers. |
| [server-lifecycle](crates/server-lifecycle) | Shared protocol-neutral server lifecycle primitives: bind configuration, drain and readiness signals, connection budgets, shutdown hooks, and metrics hooks. |
| [server-tcp](crates/server-tcp) | Shared TCP protocol runtime built on server-lifecycle: accept loop, per-connection supervision, admission budgeting, drain-aware shutdown, and handler traits. |
| [service-auth](crates/service-auth) | Shared request-auth middleware for the ecosystem's HTTP services: the generic extract -> verify -> reject -> inject plumbing plus a `Verifier` trait each service implements. Token crypto lives in `crates/claim-token`; per-resource authorization (RBAC, scope-vs-key) stays in the service handlers. The common shape lumen/keep/relay/loom each hand-roll today. |
| [service-backup](crates/service-backup) | Shared backup contract for axiom services: destination/policy schema, sink trait, local sink, and runner primitive. Services produce consistent snapshots; backup runners upload them. |
| [service-collector](crates/service-collector) | Reusable checkpointed collector runtime with quarantine and retry control. |
| [service-executor](crates/service-executor) | Bounded asynchronous execution primitives for Axiom services |
| [service-http](crates/service-http) | Shared HTTP-service policy and adapters: standard probe routes, observability compatibility, lifecycle readiness, runtime delegation, and the {error, message} envelope. |
| [service-k8s](crates/service-k8s) | Shared Kubernetes integration for Axiom services: managed-service reconciliation, leader election, CRD and workload rendering, stateful capacity planning, and resize primitives. |
| [service-mcp](crates/service-mcp) | Shared MCP stdio and streamable HTTP transport with browser security policy. |
| [service-observability](crates/service-observability) | Protocol-neutral service observability integration: structured logging, optional OTLP traces, metrics, lifecycle counters, and process-resource sampling. |
| [service-projection](crates/service-projection) | Typed projection registry, checkpoints, catch-up, rebuild, and flush runtime. |
| [storage-durable](crates/storage-durable) | Shared durable local-storage primitives: fsync policy, atomic replacement, CRC-framed logs, and sequence-named snapshot file stores. |
| [storage-object](crates/storage-object) | Shared local and cloud object-store boundary with conditional writes and versioned metadata. |
| [storage-segment](crates/storage-segment) | Immutable segment and manifest-last archive coordination. |
| [surface](crates/surface) | Renderer-neutral UI element model shared by Jet WASM, native desktop readers, renderers, and parity tools |
| [transport-h2c](crates/transport-h2c) | Shared HTTP/2 cleartext transport: client helpers, connection management, pooling, sizing, and optional per-connection HTTP/1.1 plus h2c protocol support. |
| [transport-otlp](crates/transport-otlp) | Official OTLP HTTP and gRPC transport shell. |
| [ui-runtime](crates/ui-runtime) | Renderer-neutral component runtime: hooks, fiber storage, mount, flush, and update scheduling over surface elements |

## Using a crate

Crates are consumed as git dependencies pinned to a release tag:

```toml
[dependencies]
raft-core = { git = "https://github.com/faberline/core", tag = "v0.4.13" }
```

`index-text` needs the vendored `jieba-rs` patch. Cargo only honours `[patch]`
from the root workspace, so a consumer that depends on `index-text` carries the
same patch (and a copy of `vendor/jieba-rs`) in its own root manifest:

```toml
[patch.crates-io]
jieba-rs = { path = "vendor/jieba-rs" }
```

## Working here

```sh
cargo check --workspace --all-targets
cargo test -p <crate>
```

Conventions every crate follows live in [CONTRIBUTING.md](CONTRIBUTING.md); each
crate's own `CONTRIBUTING.md` covers what that crate promises. How the
workspace is cut into contexts and layers, the glossary, the per-context domain
pages, and the release process are in [docs/](docs/README.md).
