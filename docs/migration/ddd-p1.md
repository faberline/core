# Migrating to DDD P1

P2 changes are in [ddd-p2.md](ddd-p2.md); later source paths are in
[ddd-p2-paths.md](ddd-p2-paths.md).

P1 (branch `refactor/ddd-p1`) moves core's source files into the layered
layout described in [architecture](../architecture.md) and
[ADR 0001](../adr/0001-standard-layout-and-ddd.md). It moves code and does not
change behaviour, apart from the one bug fix below.

## Rust code: nothing to change

Downstream code builds against P1 as it builds against main:

- **Public paths are unchanged.** Every crate-root `pub use` stays, and every
  old `pub mod` is kept as a compatibility facade, so `service_k8s::lease::*`,
  `cli_std::llm::v2::Topic` and the other module paths still resolve to the
  same items.
- **Features, wire types and persisted formats are unchanged.** No feature
  flag, serialized type or on-disk format was edited. The runtime sources
  that openapi-codegen embeds moved to `assets/` files read with
  `include_str!`; the generated code is byte for byte the same.
- **One bug fix.** compass's `gen::python::test_extractor::TestExtractor`
  split `assert_eq!` arguments on character positions, so an argument with a
  non-ASCII character before the split point came out cut in the wrong place
  or panicked. It now splits on byte offsets. ASCII input translates as
  before.
- **Checked:**
  - `cargo-public-api` lists the same items as main for every crate, with
    all features and with each extra feature set (service-k8s with no
    default features, `render-only`, `controller` and `certificate`;
    service-auth and service-http with no default features). Only the paths
    it displays differ: items defined in a moved module print under their
    new private path, and compass's `scope::Symbol` and `scope::SymbolKind`
    print under their existing `ScopeSymbol` and `ScopeSymbolKind` aliases.
  - `cargo test --workspace --all-features` gives the same 2032 results as
    main, plus one new compass test for the bug fix.
  - `cargo check --all-targets` in each of the 17 downstream repos, patched
    to P1, gives the same result as against main: all 17 build, with the
    same warning counts.

## What you may notice

- **Source file paths.** Files moved into
  `crates/<crate>/src/{domain,application,infrastructure,interfaces}/`, and
  files above 400 lines were split. A comment, document or script that cites a
  core source path (often with line numbers) now points at a missing or
  different file. [ddd-p1-paths.md](ddd-p1-paths.md) lists where every old
  file's items are now.
- **Paths in tool output.** A tool that prints where an item is defined can
  now show its private module path: cargo-public-api prints raft-runtime's
  `ClusterTopology::node_id` as `raft_core::domain::ids::NodeId` where it
  printed `raft_core::NodeId`, and compiler messages can do the same. Keep
  importing by the public path; the private path cannot be named from outside
  the crate.
- **Behavior contracts.** `crates/<crate>/external-contracts/` moved to
  `crates/<crate>/docs/contracts/` for claim-token, compass, surface and
  ui-runtime (ADR D10).
- **Comment markers.** The `CODEGEN-BEGIN`/`CODEGEN-END` and `HANDWRITE`
  comment markers are gone from core's `src/`. A tool that counted them in
  core will find none.
- **Test names.** Integration test modules under `tests/` keep their names.
  A unit test's libtest path follows the module it tests, so tests of moved
  code gain the layer and any new submodule: raft-runtime's
  `fenced_assignment::tests::…` is now
  `application::fenced_assignment::tests::…`. A filter on an old full unit
  test path matches nothing and runs zero tests. lumen's release-candidate CI
  runs `stateful_instance_render`, `stateful_adapter_equivalence` and
  `adversarial_recovery` by name; all three are integration test modules and
  are unchanged.

## Contributing to core

- New code goes in the layer and context the rules in
  [architecture.md](../architecture.md) give it. `ddd.toml` sets
  `[migration] enforce = true`, so the architecture checker fails on any
  error-level finding; see [operations](../operations/README.md#architecture-checker)
  for the command.
- Core now denies `clippy::mod_module_files`; every member opts in with
  `[lints] workspace = true`. A module's file is `<name>.rs` next to its
  `<name>/` directory, never `<name>/mod.rs`. This applies to core's crates
  only.

## Downstream references to core source paths

The downstream inventory found 80 references to core source files in comments,
tech-design notes, runbooks, scripts and alert rules. No code reads them, so
nothing fails; they only point readers at the wrong place.

- **Broken by P1 (2):** beam's and courier's `README.md` cite
  `crates/service-http/src/transport.rs`. That code is now in
  `crates/service-http/src/interfaces/transport/` (`serve.rs`,
  `access_log.rs`, `trace_context.rs`).
- **Already stale before P1 (78):** these cite `libs/<crate>/src/...`, the
  layout from before core moved its crates under `crates/`. Each repo's
  maintainer gets its own list with the current location of every cited file.

| Repo | References | Broken by P1 | Already stale |
|------|-----------:|-------------:|--------------:|
| beam | 1 | 1 | 0 |
| courier | 4 | 1 | 3 |
| jet | 1 | 0 | 1 |
| keep | 1 | 0 | 1 |
| loom | 1 | 0 | 1 |
| lumen | 52 | 0 | 52 |
| pgpool | 9 | 0 | 9 |
| tape | 11 | 0 | 11 |

## After P1

P2 (branch `refactor/ddd-p2`) removes the exceptions that have a planned fix
and deletes or makes permanent the compatibility facades. It is a breaking
release: `docs/migration/ddd-p2.md` will list each changed path and API with
the downstream code it affects.
