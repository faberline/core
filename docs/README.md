# core docs

Project-level documentation for the core workspace. Per-crate usage lives in
each crate's `README.md` and `llms.txt`; contributor rules live in
[`CONTRIBUTING.md`](../CONTRIBUTING.md).

| Document | What it answers |
|----------|-----------------|
| [architecture.md](architecture.md) | How the workspace is cut into bounded contexts and layers, what may depend on what, and which rule breaks are allowed and why. |
| [glossary.md](glossary.md) | Terms that mean the same thing in every crate, and terms that mean different things in different crates. |
| [domain/](domain/) | One page per context: what it models, its ports, its invariants, and its recorded exceptions. |
| [adr/](adr/) | Architecture decisions and the reasons behind them. |
| [operations/](operations/README.md) | Releasing, how downstream repos pin core, and how to run the architecture checker. |

The architecture contract itself is [`ddd.toml`](../ddd.toml) at the repository
root. It is checked by the workspace's rust-arch contract
(`scripts/meta/rust_arch_contract.py` in the workspace repo); see
[operations](operations/README.md#architecture-checker) for the command.

## Domain pages

| Group | Contexts |
|-------|----------|
| Consensus | [raft-core](domain/raft-core.md), [raft-runtime](domain/raft-runtime.md) |
| Storage | [storage-durable](domain/storage-durable.md), [storage-object](domain/storage-object.md), [storage-segment](domain/storage-segment.md), [index-text](domain/index-text.md) |
| Server runtime | [server-lifecycle](domain/server-lifecycle.md), [server-tcp](domain/server-tcp.md), [server-http](domain/server-http.md), [transport-h2c](domain/transport-h2c.md), [peer-tls](domain/peer-tls.md) |
| Service kit | [service-http](domain/service-http.md), [service-auth](domain/service-auth.md), [service-observability](domain/service-observability.md), [service-executor](domain/service-executor.md), [service-collector](domain/service-collector.md), [service-projection](domain/service-projection.md), [service-backup](domain/service-backup.md), [service-k8s](domain/service-k8s.md), [service-mcp](domain/service-mcp.md) |
| Telemetry wire | [metrics-prometheus](domain/metrics-prometheus.md), [metrics-remote-write](domain/metrics-remote-write.md), [transport-otlp](domain/transport-otlp.md) |
| Tokens | [claim-token](domain/claim-token.md) |
| CLI and tooling | [cli-std](domain/cli-std.md), [openapi-codegen](domain/openapi-codegen.md), [build-stamp](domain/build-stamp.md), [compass](domain/compass.md) |
| UI | [surface](domain/surface.md) (shared kernel), [ui-runtime](domain/ui-runtime.md) |
