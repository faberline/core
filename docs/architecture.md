# Architecture

core is a workspace of 30 library crates. Each crate is one bounded context.
This page says how the contexts are laid out, what may depend on what, and
which rule breaks are allowed and why. The machine-checked form of everything
here is [`ddd.toml`](../ddd.toml); the decisions behind it are in
[ADR 0001](adr/0001-standard-layout-and-ddd.md).

> **Migration status.** Phase P1 applied the layout below to every crate and
> set `[migration] enforce = true`, so any error-level finding fails the
> checker. Phase P2 fixed every exception that had a planned fix; each one
> left has a long-term reason (see [Exceptions](#exceptions)). See
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

Every crate takes one of four forms. The form decides where its source files
live and which rules apply to them.

| Form | Layout | Crates |
|------|--------|--------|
| **Layered** | `src/{domain,application,infrastructure,interfaces}/<concept>/`. A layer with nothing in it is not created. | cli-std, compass, openapi-codegen, raft-runtime, service-auth, service-backup, service-collector, service-http, service-k8s, service-projection, storage-segment |
| **Domain only** | `src/domain/<concept>/` | claim-token, index-text, raft-core, ui-runtime |
| **Whole-src adapter** | all of `src/` is one layer, split by concept directly under `src/` | infrastructure: build-stamp, metrics-prometheus, peer-tls, server-lifecycle, service-executor, service-observability, storage-durable, storage-object, transport-h2c. interfaces: metrics-remote-write, server-http, server-tcp, service-mcp, transport-otlp |
| **Shared kernel** | all of `src/` | surface |

A crate is whole-src when it is a technical library with no use cases of its
own: it adapts one piece of technology (a file system, a wire format, a TLS
stack, a runtime) and other contexts use all of it. Splitting such a crate into
layers would leave empty layers and hide its API behind the cross-context rule
below. [ADR 0001 D6 and D19](adr/0001-standard-layout-and-ddd.md) record which
crates are whole-src and why.

In every form, `src/lib.rs` holds only `mod` declarations and `pub use`
re-exports. A crate may also have public API modules under `src/api/` and a
composition root under `src/app/`; neither belongs to a layer (see
[Public API modules and the composition root](#public-api-modules-and-the-composition-root)).

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

A composition root may use another context's infrastructure, because it is
assembly code, not a layer. service-backup relies on this: only its `src/app`
reads a projected token through service-auth's `ProjectedTokenFile`, and the
token reaches the rest of the crate through a domain port.

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
  refresh). Only the domain layer must stay pure. Domain code that needs the
  time takes it as an argument, as `ProjectionCheckpoint::empty(now)` does, or
  reads it through a port, as service-auth's `gcp::Clock` and `k8s::Clock`
  do.

## Public API modules and the composition root

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
- Each surviving crate-root export keeps its path; D7 deletions are listed in the migration guide.

A **composition root** at `src/app.rs` and `src/app/` builds a crate's
infrastructure adapters and passes them to its application layer through the
ports the domain defines. A public entry point that needs both sits there:
for example `openapi_codegen::run`, which reads the spec file and writes the
output, and service-backup's `sink_from_destination` and
`run_admin_snapshot_backup`, which pick a sink adapter and wire the admin
snapshot transport. The composition root may use every layer of its own
crate.

`ddd.toml` lists each `src/api` and `src/app` directory under
`[assembly] modules`. Assembly code wires layers together and is not part of
any layer, so the layer rules do not apply to it.


## File size

The checker measures non-test source files under `src/`: above 400 lines is
a warning (C1), above 1000 lines is an error (C2) unless the file is listed in
`[[large_files]]` with a ceiling and a reason. A C1 warning cannot be
suppressed. After P2 no measured file is above 400 lines, and
`[[large_files]]` is empty. Integration tests under `tests/` are not
measured; their layout rules are in
[CONTRIBUTING](../CONTRIBUTING.md#test-layout--testsit-one-integration-binary-per-crate).

## Exceptions

`ddd.toml` lists every structure or dependency break (rules B1–B6) that the
checker would otherwise report as an `[[exceptions]]` entry: the rule, the
subject, the exact files, and a reason. No reason may be "TODO". P1 recorded
each break it moved as it was, with the P2 change that would remove it; P2
made those changes and deleted the entries. Every entry left is permanent,
and its reason says why. File-size warnings (C1) are not exceptions; they
stay in the report.

| Rule | Subject | Context | Why it stays |
|------|---------|---------|--------------|
| B2 | `base64` | claim-token | base64url is the token's wire encoding, fixed by the format keep and loom already exchange. The crate is a pure encoder with no I/O. |
| B2 | `jieba_rs` | index-text | The Jieba analyzer is a persisted schema choice (`Analyzer::Jieba`), and jieba-rs is its segmentation algorithm over an embedded dictionary, with no I/O. It sits behind the optional `jieba` feature. |
| B2 | `tree_sitter` | compass | compass E1: the syntax model is tree-sitter's. `ParsedFile` wraps a `tree_sitter::Tree`, and about 20,000 lines of checkers, visitors, type inference and search walk `tree_sitter::Node` on the hot path. Only the core crate is used in the domain; the grammar crates stay in infrastructure. |
| B2 | `serde_yaml`, `toml` | compass | Pure in-memory data-format codecs, like `serde_json` on the allowlist: one writes a state machine's YAML frontmatter, the other parses linted text to report TOML syntax errors. |
| B2 | `schemars` | service-backup, service-k8s | `JsonSchema` is a compile-time description of the same serde wire shape. Downstream CRD specs and statuses embed these types as-is, so their generated schemas must not change, and an interfaces copy would duplicate the wire contract. |
| B2 | `utoipa` | service-projection | `ToSchema` describes the same serde wire shape. sift serves these types as-is in its OpenAPI document under these schema names. |
| B3 | `infrastructure->application` | service-auth | `JwksSource` returns `jsonwebtoken::jwk::JwkSet`, a type outside the domain allowlist, so the port cannot move to the domain without a second copy of the RFC 7517 key set. `TokenMinter`'s error is returned by the interfaces `LoopbackProxy::next_fatal`, so moving it to the domain would make interfaces name a domain type. |
| B3 | `interfaces->domain` | compass | compass E2: the LSP server and the agent and report renderers read only the wire value objects `Diagnostic`, `DiagnosticSeverity`, `Range`, `Position` and `FileResult`, which are the agent and LSP format. Everything else reaches interfaces through the application layer. |

Some design debts are **not** checker findings, so they have no exception
entry. Each domain page lists its own under **Debts**:

- public fields that downstream code builds with struct literals, on types P2
  did not change (ADR D2). Wire types and output-only types keep public
  fields on purpose; each page names them under **Public fields kept**;
- bare `u64` / `String` ids where a newtype belongs;
- `anyhow` in public functions that are not ports (ADR D4 covers ports), and
  in `cli_std::CliModule::execute`;
- file-system reads the checker does not detect, such as compass's Markdown
  relative-link check.

## Phases

- **P1 — move, do not change.** Files moved into the layout above and files
  above 400 lines were split. Public paths, behaviour, and persisted formats
  stayed the same; downstream repos needed no code change. Every rule break
  became an exception with a reason. The last P1 commit set `enforce = true`.
  [The P1 migration guide](migration/ddd-p1.md) lists the new source paths.
- **P2 — fix what the checker requires.** Each exception with a planned fix
  was fixed and removed; compatibility facades were deleted or made
  permanent; dead code was deleted; the listed ports return `thiserror`
  errors, and the listed configuration and input types have private fields.
  The W5 contexts use separate identity, version, cursor and generation types.
  Golden tests pin every persisted and wire format the changes touched.
  [The P2 migration guide](migration/ddd-p2.md) lists each API change and the
  downstream code it affects; [ADR 0001](adr/0001-standard-layout-and-ddd.md#p2-outcome)
  records where P2 departed from the plan.
