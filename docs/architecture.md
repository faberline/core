# Architecture

core is a workspace of 30 library crates. Each crate is one bounded context.
This page says how the contexts are laid out, what may depend on what, and
which rule breaks are allowed and why. The machine-checked form of everything
here is [`ddd.toml`](../ddd.toml); the decisions behind it are in
[ADR 0001](adr/0001-standard-layout-and-ddd.md).

> **Migration status.** Phase P1 applied the layout below to every crate and
> set `[migration] enforce = true`, so any error-level finding fails the
> checker. Phase P2 removes the exceptions that have a planned fix. See
> [Phases](#phases).

## Context map

One crate is one context. A context's `depends_on` is exactly its normal
(non-dev) dependencies on other core crates; an optional dependency counts.
`surface` is the shared kernel, which any context may use without declaring it.

| Context | Form | Depends on | What it owns |
|---------|------|------------|--------------|
| [build-stamp](domain/build-stamp.md) | infrastructure | — | build.rs stamping: git sha, build time, target triple |
| [claim-token](domain/claim-token.md) | domain | — | HMAC-SHA256 scoped claim tokens |
| [cli-std](domain/cli-std.md) | layered | — | the standard CLI commands (`llm`, `upgrade`, `issue`, `connect`) and their shared types |
| [compass](domain/compass.md) | layered | — | multi-language code intelligence: parse, lint, infer, search, refactor |
| [index-text](domain/index-text.md) | domain | — | in-process full-text index with versioned snapshots |
| [metrics-prometheus](domain/metrics-prometheus.md) | infrastructure | — | lock-free metric primitives and the Prometheus text encoder |
| [metrics-remote-write](domain/metrics-remote-write.md) | interfaces | — | Prometheus Remote Write 1.0 wire codec |
| [openapi-codegen](domain/openapi-codegen.md) | layered | cli-std | typed TS / Python / Rust clients from an OpenAPI document |
| [peer-tls](domain/peer-tls.md) | infrastructure | — | peer mTLS material: loading, validation, hot rotation |
| [raft-core](domain/raft-core.md) | domain | — | the tick-driven, IO-free Raft state machine |
| [raft-runtime](domain/raft-runtime.md) | layered | cli-std, peer-tls, raft-core, server-lifecycle, storage-durable, transport-h2c | the async host that runs raft-core for a service state machine |
| [server-http](domain/server-http.md) | interfaces | server-lifecycle, server-tcp, transport-h2c | the HTTP/1.1 + h2c and TLS listener shell |
| [server-lifecycle](domain/server-lifecycle.md) | infrastructure | — | process lifecycle: phases, drain, shutdown hooks, connection budgets |
| [server-tcp](domain/server-tcp.md) | interfaces | server-lifecycle | the generic TCP accept loop |
| [service-auth](domain/service-auth.md) | layered | cli-std, metrics-prometheus | bearer-token authentication and role authorization |
| [service-backup](domain/service-backup.md) | layered | cli-std, service-auth, storage-durable, storage-object | the backup contract: destinations, policy, sinks, restore |
| [service-collector](domain/service-collector.md) | layered | storage-durable | at-least-once record collection with checkpoint and quarantine |
| [service-executor](domain/service-executor.md) | infrastructure | — | bounded concurrency, group commit, job runner |
| [service-http](domain/service-http.md) | layered | server-http, server-lifecycle, service-observability | the HTTP service kit: admission, tracing, probes, errors, proxy |
| [service-k8s](domain/service-k8s.md) | layered | cli-std, metrics-prometheus | the Kubernetes operator kit: reconcile, leases, manifests, certificates |
| [service-mcp](domain/service-mcp.md) | interfaces | service-auth | MCP server scaffolding |
| [service-observability](domain/service-observability.md) | infrastructure | metrics-prometheus, server-lifecycle | logging, tracing, service metrics and resource probes |
| [service-projection](domain/service-projection.md) | layered | storage-durable | durable, rebuildable read-model projections |
| [storage-durable](domain/storage-durable.md) | infrastructure | — | crash-safe local file-system primitives |
| [storage-object](domain/storage-object.md) | infrastructure | storage-durable | the object-store port and its local, GCS and S3 adapters |
| [storage-segment](domain/storage-segment.md) | layered | storage-object | copy-on-write paged catalog and manifest-last archives |
| [surface](domain/surface.md) | shared kernel | — | the renderer-neutral UI element tree |
| [transport-h2c](domain/transport-h2c.md) | infrastructure | cli-std, server-lifecycle | HTTP/2 cleartext client and per-connection serving |
| [transport-otlp](domain/transport-otlp.md) | interfaces | — | the OTLP HTTP and gRPC ingest shell |
| [ui-runtime](domain/ui-runtime.md) | domain | surface (kernel) | the React-style hooks runtime on top of surface |

## Crate forms

Every crate takes one of five forms. The form decides where its source files
live and which rules apply to them.

| Form | Layout | Crates |
|------|--------|--------|
| **Layered** | `src/{domain,application,infrastructure,interfaces}/<concept>/`. A layer with nothing in it is not created. | cli-std, compass, openapi-codegen, raft-runtime, service-auth, service-backup, service-collector, service-http, service-k8s, service-projection, storage-segment |
| **Domain only** | `src/domain/<concept>/` | claim-token, index-text, raft-core |
| **Whole-src domain** | all of `src/` is the domain layer | ui-runtime (until P2 moves it to `src/domain/`) |
| **Whole-src adapter** | all of `src/` is one layer, split by concept directly under `src/` | infrastructure: build-stamp, metrics-prometheus, peer-tls, server-lifecycle, service-executor, service-observability, storage-durable, storage-object, transport-h2c. interfaces: metrics-remote-write, server-http, server-tcp, service-mcp, transport-otlp |
| **Shared kernel** | all of `src/` | surface |

A crate is whole-src when it is a technical library with no use cases of its
own: it adapts one piece of technology (a file system, a wire format, a TLS
stack, a runtime) and other contexts use all of it. Splitting such a crate into
layers would leave empty layers and hide its API behind the cross-context rule
below. [ADR 0001 D6 and D19](adr/0001-standard-layout-and-ddd.md) record which
crates are whole-src and why.

In every form, `src/lib.rs` holds only `mod` declarations and `pub use`
re-exports.

## Layers and dependency direction

Inside one context, a layer may use itself and the layers below it:

| Layer | May use |
|-------|---------|
| domain | nothing else in the context |
| application | domain |
| infrastructure | domain |
| interfaces | application |

- **domain** holds the model: value objects, entities, state machines, domain
  services, and the ports (traits) that the model needs from the outside.
- **application** holds use cases that drive the model through its ports, and
  the types other contexts are meant to consume.
- **infrastructure** implements ports against real technology: files,
  networks, clocks, codecs, third-party APIs.
- **interfaces** is the inbound edge: HTTP handlers, CLI output, daemon
  protocols, `llm` help topics. It calls use cases and speaks in the types the
  application layer defines; it does not name domain types itself.

### Across contexts

- A context may use another only if it lists it in `depends_on`. The graph has
  no cycles.
- A **domain layer never uses another context.** The shared kernel is the only
  outside code a domain layer may name.
- Another context may use a layered context **only through its application
  layer** (or `domain::events`). The application layer is the context's
  published language: the types and functions it offers to other contexts. A
  type that another core context needs is therefore defined in the application
  layer, even when it is a plain value.
- A whole-src or domain-only context has no application layer, so its whole
  public API is open to its dependents.
- The shared kernel uses no context.

The checker resolves `pub use` chains to the file that defines an item, so a
re-export does not change where a type counts as living. Moving the definition
is the only way to change it.

Two cross-context uses do not fit these rules yet: service-backup reads a
projected token through service-auth's infrastructure `ProjectedTokenFile`, and
storage-segment's domain carries storage-object's `ObjectStoreError` and
`ObjectVersion`. They are listed as exceptions with their fix; see
[Exceptions](#exceptions).

## Domain purity

The domain layer and the shared kernel are held to the `[policy.domain]` rules
in `ddd.toml`:

- **Allowed outside crates:** `std`, `core`, `alloc`, `thiserror`, `serde`,
  `serde_json`, `chrono`, `time`, `uuid`, `sha2`, `hex`, `regex`, `bytes`.
- **Denied paths:** file system, network, process, environment, OS, standard
  streams, thread spawn and sleep, and every "read the clock" or "make a random
  id" call (`SystemTime::now`, `Instant::now`, `Utc::now`, `Uuid::new_v4`, …).
- **Denied macros:** `println!`, `print!`, `eprintln!`, `eprint!`, `dbg!`.

Two points are deliberate:

- **serde stays in the domain.** In core, many domain types *are* the wire or
  persisted format: Raft messages and persisted state, catalog pages whose id
  is the sha256 of their JSON, text-index snapshots, CRD types, the
  `cclab.llm.v2` protocol. Mirroring them into DTOs would double the code and
  risk byte drift in stored data. Policy allows serde in the domain for this
  reason.
- **The application layer may use tokio and read the clock.** Use cases in
  core are async orchestration (the Raft host, lifecycle controllers, token
  refresh). Only the domain layer must stay pure.

## Public API modules

Downstream repos import core by module paths such as
`service_k8s::lease::LeaseConfig` or `cli_std::llm::v2::Topic`. Moving files
into layers would change those paths, so P1 kept every one of them: each old
`pub mod` became a **compatibility facade**, a file at `src/compat/<old>.rs`
holding only `pub use` lines. P2 settled every facade:

- A facade whose names are all exported at the crate root, as the same items,
  is deleted. Callers import those names from the root;
  [the P2 migration guide](migration/ddd-p2.md) lists each deleted path.
- Every other facade is a permanent public module at `src/api/<name>.rs` with
  the same path. It still holds only `pub use` lines (nested public paths are
  inline `pub mod` blocks inside it). `lib.rs` declares `mod api;` and
  `pub use api::<name>;`.
- Every `pub use` at a crate root stays.
- `src/api` is declared under `[assembly] modules` in `ddd.toml`: it wires
  layers together and is not itself part of any layer.

## File size

The checker measures non-test source files under `src/`: above 400 lines is
a warning (C1), above 1000 lines is an error (C2) unless the file is listed in
`[[large_files]]` with a ceiling and a reason. P1 splits every file above 400
lines. A C1 warning cannot be suppressed; the few files that stay above 400
lines until P2 remain visible as warnings. Integration tests under `tests/` are not
measured; their layout rules are in
[CONTRIBUTING](../CONTRIBUTING.md#test-layout-testsit-one-integration-binary-per-crate).

## Exceptions

`ddd.toml` lists every structure or dependency break (rules B1–B6) that the
checker would otherwise report as an `[[exceptions]]` entry: the rule, the
subject, the exact files, and a reason. Each reason says either why the break
is permanent or which P2 change removes it. No reason may be "TODO". File-size
warnings (C1) are not exceptions; they stay in the report.

The categories are:

| Category | Rule | Where | Why it is allowed | Fate |
|----------|------|-------|-------------------|------|
| A crate is not split yet | B1 naming | surface, ui-runtime | These crates are SPEC-MANAGED; splitting them waits on confirming that nothing regenerates them (ADR D8, D18). Their `lib.rs` files (423 and 693 lines) also stay C1 warnings. | P2 splits them and removes the exceptions. |
| A domain type is bound to a crate that is part of its format | B2 | claim-token (`base64`), index-text (`jieba_rs`), compass (`tree_sitter`) | The crate is the model's own vocabulary: the token's wire encoding, a persisted analyzer, a syntax tree. It does no I/O; wrapping it would mean rewriting the model. | Kept with a long-term reason. |
| A domain type carries a schema or trait crate | B2 | service-backup and service-k8s (`schemars`), service-projection (`utoipa`), service-k8s certificate ports (`futures`, `rcgen`) | Downstream CRDs and OpenAPI documents embed these types, and their generated schemas must not change; the certificate ports return `BoxFuture` and build the CSR in place. | P2 moves schemas to interfaces types with the same output, uses std's boxed future, and puts key generation behind a port. Anything that must stay gets a long-term reason. |
| A domain port or parser returns `anyhow` | B2 `anyhow` | openapi-codegen, service-auth, service-backup, service-projection, compass (`SearchIndex` bytes) | The error type is part of the signature (ADR D4); P1 only moves files. | P2 gives each a `thiserror` error type. |
| A domain type reads the clock or the file system | B2 | service-projection (`Utc::now` in an empty checkpoint), service-k8s (`now_rfc3339`), compass (`Instant::now` in the dirty-file tracker and the six search modes) | P1 only moves files. | P2 adds `Clock` and file-system ports. |
| A domain helper uses a library outside the allowlist | B2 | compass (`regex_lite`, `serde_yaml`, `toml`, `tracing`, `bincode`) | Pattern matching, frontmatter output, a TOML syntax check, a log line and index bytes; none of them does I/O. P1 only moves files. | P2 switches to `regex`, returns rejected rules as data, and moves serialisation behind infrastructure; a crate that must stay gets a long-term reason. |
| Domain code calls its own adapter | B3 `domain->infrastructure` | compass (the Python inferencer resolves imports and loads stubs; semantic search and the refactoring AST cache parse source) | The engine loads or parses what it needs on demand; separating that is a rewrite. | P2 passes parsed files and resolved modules in from application, or adds domain ports. |
| Application calls its own adapter | B3 `application->infrastructure` | storage-segment (paged catalog, archive), service-projection (file state), service-backup (sink, admin snapshot), cli-std (kubectl, GitHub, courier, prompt, self-install), service-k8s (certificate Secret layout), raft-runtime (the host holds the store and the peer transport), compass (use cases build the parser, walk files, hold the disk cache, and dispatch over the codegen trait) | The use case and its I/O are interleaved; separating them is a rewrite, not a move. | P2 adds the port and wires the adapter in assembly code. |
| Adapters use application types | B3 `infrastructure->application` | service-auth (JWKS and introspection sources, Kubernetes review backend, token minter), raft-runtime (peer RPC client, store, topology reader), compass (the daemon client uses the daemon config and protocol) | service-auth's ports use `#[async_trait]` and `jsonwebtoken` types, which the domain allowlist does not include, so they sit in the application layer. raft-runtime has no domain layer (raft-core is its model), so its own value types live in the application layer. | P2 moves the ports and value types to a domain layer. |
| Interfaces reach past the application layer | B3 `interfaces->domain`, `interfaces->infrastructure` | openapi-codegen CLI, service-backup `llm` topic, service-k8s operator (reconcile, leader election), raft-runtime peer envelope, compass (LSP server, daemon, reporters) | The module is one inbound-and-outbound unit today, or the domain value is itself the wire format. | P2 moves the sequence into an application use case. Where the value is the wire format, it is kept with a long-term reason. |
| Cross-context use outside the published language | B4 | service-backup → service-auth `ProjectedTokenFile`; storage-segment domain → storage-object `ObjectStoreError`, `ObjectVersion` | The consumer needs a type that its provider does not publish through an application layer, or a domain type wraps another context's type. | P2 publishes the token source through service-auth's application layer, and gives storage-segment's domain its own error variant and version type. |

Some design debts are **not** checker findings, so they have no exception entry.
They are tracked for P2 and listed on each domain page:

- public fields that downstream code builds with struct literals (ADR D2);
- bare `u64` / `String` ids where a newtype belongs;
- ports outside a domain layer that return `anyhow::Error` (ADR D4; in a
  domain layer `anyhow` is a B2 finding and has an exception entry);
- file-system reads the checker does not detect, such as compass's Markdown
  relative-link check;
- dead or duplicated code (ADR D7).

## Phases

- **P1 — move, do not change.** Files move into the layout above and files
  above 400 lines are split. Public paths, behaviour, and persisted formats are
  unchanged; downstream repos need no code change. Every rule break is an
  exception with a reason. The last P1 commit sets `enforce = true`.
- **P2 — fix what the checker requires.** Each exception with a planned fix is
  fixed and removed; compatibility facades are deleted or made permanent; the
  public-field, newtype and error-type debts above are paid. Each API change is
  listed in `docs/migration/` with the downstream code it affects.
