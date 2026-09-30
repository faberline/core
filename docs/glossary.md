# Glossary

Two parts: terms that mean the same thing in every core crate, and terms that
mean different things in different crates. Terms used inside only one context
are defined on that context's page under [domain/](domain/).

Nothing here renames code. When a name is ambiguous, docs and reviews use the
qualified name from the second table; the code keeps its current name.

## Shared terms

### Architecture

| Term | Meaning |
|------|---------|
| **context** | A bounded context: one crate, with one model and one vocabulary. See [architecture](architecture.md#context-map). |
| **layer** | One of `domain`, `application`, `infrastructure`, `interfaces` inside a layered context. |
| **port** | A trait the domain or application layer defines for something it needs from outside (a clock, a store, a transport). |
| **adapter** | An implementation of a port against real technology, or an inbound handler that turns a protocol request into a use-case call. |
| **published language** | The types and functions a context offers to other contexts. In a layered context it is the application layer. |
| **shared kernel** | Code every context may use without declaring a dependency. In core it is `surface`. |
| **assembly** | Code that wires layers together and belongs to no layer: `lib.rs` and `src/api/`. |
| **api module** | A public module under `src/api/` holding only `pub use` lines. It keeps a module path whose names the crate root does not re-export. |
| **compat facade** | P1's file under `src/compat/` holding only `pub use` lines, so an old public module path kept working after its code moved. P2 deleted each one or made it an api module. |
| **exception** | A recorded rule break in `ddd.toml`, with the exact files and a reason. |

### Ecosystem

| Term | Meaning |
|------|---------|
| **downstream** | A repo that depends on core by git tag (lumen, sift, jet, pgpool, …). |
| **h2c** | HTTP/2 over cleartext TCP with prior knowledge. The transport between faberline services and between Raft peers. |
| **peer** | Another replica of the same service, reached over mTLS on the replication port. |
| **bearer token** | The `Authorization: Bearer <token>` credential every service accepts; see the [service auth contract](../CONTRIBUTING.md#service-auth-one-bearer-token-contract). |
| **drain** | Stop admitting new work, let in-flight work finish within a deadline, then stop. |
| **llm topic** | A named block of help text a CLI prints under `<tool> llm <topic>`, written for an agent to read. |
| **tag** | A lightweight git tag `vX.Y.Z` on core; downstream repos pin core by tag. See [operations](operations/README.md). |

## Same word, different meaning

Each row is one word. Each meaning gets a qualified name for use in prose.

| Word | Qualified name | Meaning | Context |
|------|----------------|---------|---------|
| generation | **directory generation** | A numbered directory that the `CURRENT` pointer switches to atomically. | storage-durable |
| | **config generation** | `ConfState.generation`: a counter bumped on every membership change. | raft-core |
| | **store generation pin** | `GenerationPin`: a hold on one durable log-store generation, named by a 32-byte digest, so its files stay mapped while read. | raft-runtime |
| | **TLS reload generation** | A counter bumped on each successful TLS material reload. | peer-tls, raft-runtime `PeerTransport` |
| | **lifecycle generation** | A counter bumped on each lifecycle phase transition. | server-lifecycle |
| | **source generation** | The generation of the source a projection was built from. | service-projection |
| | **object generation** | GCS's object generation, surfaced as `ObjectVersion`. | storage-object |
| snapshot | **Raft snapshot** | The state-machine image that replaces a log prefix. | raft-core, raft-runtime |
| | **snapshot file** | A sequence-named file in a snapshot store. | storage-durable |
| | **index snapshot** | `TextIndexSnapshot`: the versioned JSON image of a text index. | index-text |
| | **surface snapshot** | `SurfaceSnapshot`: a serializable UI element tree. | surface |
| | **metrics snapshot** | `LifecycleMetricsSnapshot`, `TlsListenerSnapshot`: point-in-time counters. | service-observability, server-http |
| | **projection snapshot** | The persisted state of a projection. | service-projection |
| | **admin snapshot** | The body a service returns from `/admin/backup` for backup upload. | service-backup |
| commit | **Raft commit** | An entry index known to be on a quorum. | raft-core |
| | **generation commit** | Switching `CURRENT` to a new directory generation. | storage-durable |
| | **archive commit** | Writing the manifest last, after every segment it names. | storage-segment |
| | **source commit** | Acknowledging to a collector source that records up to a cursor are durable. | service-collector |
| checkpoint, cursor, quarantine | **collector checkpoint / cursor / quarantine** | Where a collector resumes, and where undeliverable records go. | service-collector |
| | **projection checkpoint / cursor / quarantine** | How far a projection has applied, and where unreadable state goes. | service-projection |
| Role | **Raft role** | `Role` (follower, candidate, leader) in raft-core; `RaftRole` in raft-runtime's status view adds learner. | raft-core, raft-runtime |
| | **auth role** | `Role`: `Read`, `Write`, `Admin`, with `Write` ⊇ `Read`. | service-auth |
| | **connect role** | cli-std's deliberate copy of the auth role, kept to avoid a dependency cycle. | cli-std |
| | **RBAC role** | A Kubernetes `Role` object rendered into manifests. | service-k8s |
| | **a11y role** | The accessibility role of a UI element. | surface |
| admission | **proposal admission** | `AdmissionPermit`: memory the state machine admits for a proposal, held by the host from index allocation until apply. | raft-runtime |
| | **learner admission** | `AdmissionRefused`: why a leader refused to add a learner. | raft-runtime |
| | **HTTP admission** | Token-bucket and weighted admission of requests. | service-http |
| | **lifecycle admission** | `admission_open`: whether the process accepts new work in its current phase. | server-lifecycle |
| | **connection admission** | `ConnectionBudget`: the cap on concurrent connections. | server-lifecycle, server-tcp |
| | **stream admission** | Admitting a new HTTP/2 stream on a pooled connection. | transport-h2c |
| backpressure | **capacity backpressure** | `CapacityLevel::Backpressure`: a storage capacity level. | storage-durable |
| | **proposal backpressure** | `ProposalBackpressure`: a proposal refused because the log is over budget. | raft-runtime |
| epoch, term | **Raft term** | `Term`: the election epoch. | raft-core |
| | **assignment epoch** | `AssignmentEpoch`: the fence for a shard assignment. | raft-runtime |
| | **conformance epoch** | The epoch field of a conformance-test envelope. | raft-runtime |
| membership | **Raft membership** | `Membership`: voters and learners. | raft-core |
| | **membership phase** | `MembershipPhase`: where a joint-consensus change stands. | raft-runtime |
| | **membership policy** | `MembershipPolicy`: a validator for topology changes. | raft-runtime |
| lease | **leader lease** | A Kubernetes `Lease` used for leader election. | service-k8s |
| | **command lease** | `CommittedCommandLease`: read-only mapped bytes of one committed command. | raft-runtime |
| | **connection lease** | A checked-out connection from the h2c manager. | transport-h2c |
| | **concurrency lease** | `ConcurrencyLease`: a held slot of weighted admission. | service-http |
| Source | — | Every `*Source` is a port that yields data: `JwksSource`, `TokenSource` (service-auth), `MaterialSource` (peer-tls), `RegistrySource`, `ServerConfigSource` (server-http), `CollectorSource` (service-collector), `ProjectionSource` (service-projection). Always use the full name. | several |
| fingerprint | **token fingerprint** | The first 6 bytes of a token's sha256, for logs. | service-auth |
| | **certificate fingerprint** | The full sha256 of a leaf certificate. | peer-tls |
| Clock | **seconds clock / millisecond clock** | service-auth has two clock ports, one per unit. Other crates will gain clock ports in P2; each is named with its context. | service-auth |
| Topic | **llm topic (v1)** | `cli_std::llm::Topic`: a static help topic. | cli-std |
| | **llm topic (v2)** | `cli_std::llm::v2::Topic`: a topic in the `cclab.llm.v2` JSON protocol. | cli-std |
| Application | **application layer** | The DDD layer. | all |
| | **MCP application** | `McpApplication`: the trait an MCP server implements. | service-mcp |
| trace layer | — | `trace_layer` exists in both server-http and service-http with different behaviour. Always qualify with the crate. | server-http, service-http |

Other words that recur with context-specific meanings — `Rejection`, `Target`,
`Version`, `Sample`, `Label`, `RuntimeConfig`, `manifest`, `readiness`,
`Scope`, `RetentionPolicy`, `Action`, `Outcome`, `Policy` — are defined on the
domain page of the context that uses them. Qualify them with the context in
any text that spans crates.
