# ADR 0001: Standard layout and DDD layers

- **Status:** Accepted. P1 in progress on `refactor/ddd-p1`; P2 follows on
  `refactor/ddd-p2`.
- **Date:** 2026-09-29
- **Applies to:** every crate under `crates/`. `vendor/` is not a workspace
  member and is out of scope.

## Context

Every faberlines Rust repo is being brought under one architecture contract,
checked by the workspace repo's `scripts/meta/rust_arch_contract.py` against a
`ddd.toml` at the repo root. Before this change core had no `ddd.toml`. A
first check without one, on main at `ec842d6`, reported 69 errors and 318
warnings: 33 source files above 1000 lines (C2), 30 crates whose `src/` did
not follow the layer naming (B1), six missing `docs/` entries (A5), 108 files
above 400 lines (C1), 209 `lib.rs` items that were not `mod` or `use` (B5), and
the missing `mod_module_files` lint (A3).

core is also the most depended-on repo in the ecosystem: 17 downstream repos
pin it by git tag, and several of them build core types with struct literals,
glob-import core modules, or run core tests by name in their CI. A layout
change that moved public paths would break them all at once.

## Decision

Adopt the layered layout of [architecture.md](../architecture.md) in two
phases, and record every rule break that is not fixed yet as a `ddd.toml`
exception with a reason. The numbered decisions below are the ones made while
mapping the 30 crates; the numbers are kept so that commit messages and review
threads can cite them.

### Phasing

- **D1 — Two phases.** P1 moves and splits files only: public paths,
  behaviour and persisted bytes stay the same, so downstream repos need no code
  change. P2 makes the API and semantic changes that the checker requires, each
  listed in a migration guide with the downstream code it affects.
- **D13 — 400-line target.** P1 splits every `src/` file above 400 lines, so
  the C1 warnings go away as well as the C2 errors. There is no smaller target.
  A function too long to fit after the move is split in its own
  extract-function commit rather than listed anywhere: C1 cannot be
  suppressed, and `[[large_files]]` accepts only files above 1000 lines.
- **D17 — No one-concept-per-file rule.** A file that is already on its own and
  at most 400 lines moves as it is and is not merged with others. Packing
  adjacent small concepts up to about 400 lines applies only to code split out
  of a large file.
- **D12 — Integration tests are not split.** The checker measures only `src/`.
  Large modules under `tests/` stay as they are and need no exception.

### Where code goes

- **D6 — Technical libraries are whole-src adapters.** storage-durable,
  storage-object, transport-h2c, service-executor and service-observability
  are not split into layers. Each adapts one technology and its dependents use
  all of it. The same holds for build-stamp and metrics-prometheus
  (infrastructure) and for metrics-remote-write, server-http, server-tcp,
  service-mcp and transport-otlp (interfaces).
- **D19 — server-lifecycle and peer-tls are whole-src infrastructure too.** The
  first mapping made both layered. The checker lets another context use a
  layered context only through its application layer, and both crates are
  consumed whole: raft-runtime, server-tcp, server-http, service-http,
  service-observability and transport-h2c use server-lifecycle's phases,
  budgets and signals; raft-runtime uses peer-tls's material and reload types.
  Neither crate has use cases of its own. Making them whole-src removes the
  exceptions the layered form would have needed, and matches D6.
- **D19 — A type another core context needs lives in the application layer.**
  That layer is the context's published language. Two consequences:
  - cli-std's v1 `llm` help-topic model (`Topic`, `SectionedTopic`,
    `TopicSection`, `render_sectioned`, `assert_topics_render`) is in cli-std's
    application layer, because six other contexts build their `llm` topics
    with it.
  - service-auth stays layered. Its `bearer_token` helper is in the
    application layer, so service-mcp uses it without an exception.
    service-backup's use of `k8s::ProjectedTokenFile` (infrastructure) does
    not fit yet; it is a B4 exception in P1, and P2 publishes a token source
    through service-auth's application layer.
- **Interfaces use the application layer only.** This is the checker's rule
  (`interfaces -> application`). It resolves `pub use` chains to the defining
  file, so an interfaces module that names a domain type through a re-export is
  still a finding. Such cases are fixed by placement where possible, and
  otherwise recorded as B3 `interfaces->domain` exceptions.
- **raft-runtime has no domain layer.** Its model is raft-core, a domain-only
  context that other contexts may use in full. raft-runtime's code is
  application (the host and its use cases), infrastructure (stores, the peer
  transport) and interfaces (`llm`, status views).
- **surface is the shared kernel.** It depends on no other crate, and
  ui-runtime builds on it. As the kernel, surface may be used by any context
  without a `depends_on` entry.
- **D14 — cli-std stays one crate.** It holds several small sub-contexts
  (`llm`, `upgrade`, `issue`, `connect`, `registry`, …). Policy sets
  `one_context_per_crate = false`, so this is not an exception.
- **compass #1 — compass is not split.** `spec`, `gen` and `schemas` are a
  different vocabulary from code intelligence, but they stay inside compass as
  their own concept directories. Whether they become a crate is a separate
  decision.

### Purity

- **D3 — serde stays in the domain.** Many domain types are themselves the wire
  or persisted format: Raft messages and persisted state, catalog pages whose
  id is the sha256 of their JSON, text-index snapshots, CRD types, the
  `cclab.llm.v2` protocol, compass's diagnostic and semantic-model caches.
  Policy allows `serde` in the domain; no DTO layer is added.
- **D5 — The application layer may use tokio and read the clock.** Only the
  domain layer must be pure. Direct clock and file-system reads that sit in a
  domain layer are B2 exceptions in P1; P2 adds `Clock` and file-system ports.
- **compass #3 / E1 — tree-sitter in compass's domain.** About 20,000 lines of
  node-walking code use `tree_sitter::Node` directly on the hot path. Wrapping
  it would be a rewrite. The `tree-sitter` core crate is a recorded B2
  exception; the grammar crates (`tree-sitter-*`) belong in infrastructure.
- **compass E2–E9** are recorded per item on the
  [compass page](../domain/compass.md): serde value objects that are the agent
  and cache format (E2–E4), `CodeGenerator` taking `serde_json::Value` (E5),
  `regex_lite` in custom lint (E6), plain public value fields (E7), `PathBuf` as
  file identity (E8), and the `SchemaRegistry` global (E9).
- **compass #4 — `Range::from_node` stays an inherent method in P1.** It has
  221 call sites. P2 turns it into an extension trait in infrastructure.

### What P1 does not change

- **D2 — Public fields that downstream code builds with struct literals.**
  `ToolInfo`, `llm::Topic`, `GenOptions`, `HostConfig`, `ClusterDims`,
  `Membership`, `PeerTlsConfig`, `TokenClaims`, `ProjectionDescriptor`,
  `CatalogEntry`, `BindConfig`, `TcpServerConfig`, `ShutdownDeadline` and
  others. Making them private breaks lumen, sift, jet, pgpool and others, so it
  is a P2 change with validated constructors. The checker does not report it,
  so it has no exception entry; each domain page lists it.
- **D4 — Ports that return `anyhow::Error`.** Downstream repos implement
  `RaftStateMachine`, `Projection`, `CollectorSource`, `ObjectStore`,
  `TcpHandler`, `OtlpConsumer`, `McpApplication` and others. P2 replaces
  `anyhow` in ports with `thiserror` error types; `async_trait` stays. `anyhow`
  is not on the domain allowlist, so where a port or parser sits in a domain
  layer (openapi-codegen, service-auth, service-backup, service-projection) P1
  lists it as a B2 exception; elsewhere the checker does not report it.
- **D7 — Dead and duplicated code moves as it is.** Deleting is an API change.
  The list — `RaftTransport`, the `SegmentStore` trait, the duplicate
  `HttpServerReport` / `TcpServerReport`, the duplicate `is_safe_method`,
  service-backup's lenient `fetch_admin_snapshot`, and compass's duplicates
  (compass #2) — is removed in P2.
- **compass #5 — Legacy names stay.** Argus, Lens, `lens_error` and `cclab_lens`
  are not renamed.
- **D15 — raft-core's timing constants move to their own file.** The constants
  keep their public path through a `pub use`; raft-runtime's
  `shutdown_handoff` test, which parses the constant source with `syn`, parses
  the new file.
- **Public paths.** Every crate-root `pub use` stays, and every old `pub mod`
  becomes a compatibility facade under `src/compat/` (see
  [architecture](../architecture.md#public-api-modules)).
  Special cases that the facades must preserve exactly: lumen glob-imports
  `service_k8s::lease::*`; jet re-exports `ui_runtime::*`;
  `cli_std::registry::CLI_MODULES` is a `linkme` distributed slice used through
  its path; lumen's release workflow runs core's `stateful_instance_render`,
  `stateful_adapter_equivalence` and `adversarial_recovery` test modules by
  name, so those modules are not renamed.

### Documents and markers

- **D8 — Stale generator markers.**
  - `CODEGEN-BEGIN` / `CODEGEN-END` and `HANDWRITE` comments are removed when
    their file is split in P1 (compass #6). The tool that wrote them is gone,
    and a split would cut their pairs apart. cli-std's artifact code parses
    `# SPEC-MANAGED:` and `# CODEGEN-BEGIN` lines inside YAML; that is
    behaviour, not a marker, and is not touched.
  - The `SPEC-MANAGED` markers in surface and ui-runtime, and their
    `tech-design/` directories, are handled in P2: first confirm that no tool
    regenerates these files, then split both crates into module form, remove
    the markers, and move `tech-design/` to `crates/<c>/docs/design/`.
- **D18 — surface and ui-runtime are not split in P1.** Their layout is a
  recorded B1 exception until P2 (D8). Their `lib.rs` files (423 and 693
  lines) stay C1 warnings; a warning cannot be suppressed, and splitting them
  is part of D8.
- **D10 — Behaviour contracts move under the crate's docs.** The 12
  `external-contracts/behavior/*.md` files and the four `ec.lock` files move
  to `crates/<c>/docs/contracts/` for claim-token, compass, surface and
  ui-runtime. The test headers and `aw.toml` entries that name them are
  updated. CONTRIBUTING already retired `external-contracts/` as a place for
  authored contracts; the moved files are kept as history, and new cases go
  into `tests/`.
- **D9 — Crate STATUS and ROADMAP stay beside the crate README (deferred).**
  The plan was to move them into `crates/<c>/docs/`. It is not done: the
  workspace's `project_docs_contract.py` validates `README.md`, `STATUS.md`
  and `ROADMAP.md` as one set in one project directory, and the checker's
  root-markdown rule (A4) applies only to the repository root, not to crate
  directories. Moving them would break the first check and satisfy no rule.
- **D11 — Other markdown stays.** Template and fixture READMEs remain next to
  what they describe.
- **D16 — The exception list is `ddd.toml`.** Exceptions are `[[exceptions]]`
  entries with the rule, subject, exact file paths and a reason;
  [architecture.md](../architecture.md#exceptions) groups them by category.
- **B6 — Binary directories.** The checker allows modules under
  `src/bin/<name>/` beside a thin `main.rs`. core has no binary targets, so
  this does not apply here.

### P2 scope

P2 fixes what the checker requires and pays the tracked debts; it does not
chase purity beyond that.

- serde stays in the domain (D3); tokio and clock reads stay in the
  application layer (D5).
- Each exception with a planned fix is fixed and deleted. The checker's
  ratchet (R1, `--base refactor/ddd-p1`) rejects any new exception.
- D2 fields become private with validated constructors; bare ids become
  newtypes, with golden tests proving that serialized bytes do not change.
- D4 ports return `thiserror` errors; D7 code is deleted; dependency-direction
  exceptions are fixed with ports and assembly wiring.
- A compatibility facade is deleted when every name it exports is also
  reachable at the crate root without a clash. The rest become permanent
  public modules under `src/api/<name>.rs` with the same path.

## Consequences

- P1 is invisible to downstream code. Only documents that cite core `src/`
  file paths go stale; they are listed in `docs/migration/ddd-p1.md`.
- P2 is a breaking release. Each affected downstream repo gets its own list of
  changed paths and call sites, and changes itself when it upgrades its core
  tag.
- Until P2 lands, `ddd.toml` carries exceptions. Each one names the exact files
  and either a long-term reason or its P2 fix, so the list can only shrink.
- Contributors place new code by the rules in
  [architecture.md](../architecture.md); the checker enforces them once
  `[migration] enforce = true`.
