# Compass

## Brief

Compass is the cclab code-intelligence library for navigating, checking,
searching, refactoring, generating, and incrementally watching codebases.

It exposes Rust APIs for tree-sitter parsing, language-specific linting,
semantic/type analysis, code search, refactoring operations, spec parsing,
code generation, and the Argus daemon/watch stack. The configured smoke gate
covers the library unit suite plus doctests; production readiness still depends
on semantic TD and traceability closure.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Codebase Check And Lint Pipeline | - | parser, checker, diagnostic, and output smoke gate passes |
| Semantic Navigation Search And Refactoring | - | symbol, type, search, PDG, and refactoring smoke gate passes |
| Spec Parsing And Code Generation | - | parser/generator smoke gate passes |
| Daemon Watch And Incremental Analysis | - | daemon, watch, and incremental analysis smoke gate passes |

### Codebase Check And Lint Pipeline

Compass can parse source files, dispatch language-specific checkers, return
diagnostics, and emit agent-readable reports across supported code and document
formats.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `check_paths`, `check_paths_with_propagation`,
  `LintConfig`, `FileResult`, `CheckerRegistry`, `Checker`, `Diagnostic`,
  `Reporter`; Modules: `syntax`, `lint`, `output`
- Gate — behavior: `cargo test -p compass` - configured parser, checker,
  diagnostic, and output smoke gate
- Gate: `cargo test -p compass`
- Source: `crates/compass/src/application/check/check_paths.rs`,
  `crates/compass/src/domain/lint/checker.rs`,
  `crates/compass/src/domain/lint/registry.rs`,
  `crates/compass/src/domain/syntax/parsed_file.rs`,
  `crates/compass/src/infrastructure/syntax/multi_parser.rs`,
  `crates/compass/src/interfaces/output/agent.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Multi-language parser and checker dispatch contract | epic | - | `cargo test -p compass`; crates/compass/src/application/check/check_paths.rs; crates/compass/src/domain/lint/checker.rs; crates/compass/src/domain/lint/registry.rs |
| Agent diagnostic output contract | epic | - | `cargo test -p compass`; crates/compass/src/interfaces/output/agent.rs; crates/compass/src/interfaces/output/reporter.rs |

### Semantic Navigation Search And Refactoring

Compass provides agent-facing navigation primitives for symbol outlines,
propagated type and hover answers, dependency graphs, semantic search,
PDG-style impact analysis, and structured refactoring operations.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `outline`, `outline_parsed`, `type_at`, `hover`,
  `SemanticSearchEngine`, `RefactoringEngine`, `DeepTypeInferencer`,
  `PropagationPipeline`; Modules: `semantic`, `graph`, `type_inference`
- Gate — behavior: `cargo test -p compass` - configured semantic, type
  inference, search, and refactoring smoke gate
- Gate: `cargo test -p compass`
- Source: `crates/compass/src/application/check/pipeline.rs`,
  `crates/compass/src/domain/semantic_search/engine.rs`,
  `crates/compass/src/domain/semantic_search/query.rs`,
  `crates/compass/src/domain/type_refactoring/engine.rs`,
  `crates/compass/src/domain/semantic.rs`,
  `crates/compass/src/domain/python_inference/inferencer.rs`,
  `crates/compass/src/domain/cross_file/inferencer.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Symbol outline and propagated type query contract | epic | - | `cargo test -p compass`; crates/compass/src/application/check/pipeline.rs; crates/compass/src/application/outline/function_outline.rs |
| Semantic search and graph query contract | epic | - | `cargo test -p compass`; crates/compass/src/domain/semantic_search/engine.rs; crates/compass/src/domain/semantic_search/query.rs; crates/compass/src/domain/semantic/pdg.rs |
| Structured refactoring contract | epic | - | `cargo test -p compass`; crates/compass/src/domain/type_refactoring/engine.rs |

### Spec Parsing And Code Generation

Compass parses structured specifications such as JSON Schema, OpenAPI,
AsyncAPI, Mermaid, and state-machine definitions, then provides generator
traits and registry-backed generators for Python and Rust code targets.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `GeneratorRegistry`, `CodeGenerator`, `GenContext`,
  `GeneratedCode`, `TechStack`, `StateMachineValidator`,
  `MermaidPlusGenerator`; Modules: `spec`, `gen`
- Gate — behavior: `cargo test -p compass` - configured spec parser and
  generator smoke gate
- Gate: `cargo test -p compass`
- Source: `crates/compass/src/domain/spec/ir.rs`,
  `crates/compass/src/infrastructure/spec_import/`,
  `crates/compass/src/infrastructure/codegen/traits.rs`,
  `crates/compass/src/application/codegen/registry.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Spec parser and state-machine validation contract | epic | - | `cargo test -p compass`; crates/compass/src/domain/spec/ir.rs; crates/compass/src/infrastructure/spec_import/; crates/compass/src/domain/spec/statemachine.rs |
| Python and Rust generator registry contract | epic | - | `cargo test -p compass`; crates/compass/src/infrastructure/codegen/traits.rs; crates/compass/src/application/codegen/registry.rs |

### Daemon Watch And Incremental Analysis

Compass can run a local Argus analysis daemon, track file changes, maintain
dependency-aware dirty-file sets, bridge filesystem watcher events into
incremental analysis, and serve JSON-RPC code-intelligence requests.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `ArgusDaemon`, `DaemonClient`, `DaemonConfig`,
  `RequestHandler`, `FileWatcher`, `WatchConfig`, `WatchEvent`,
  `IncrementalUpdateManager`, `DirtyFileTracker`, `DependencyGraph`,
  `WatchBridge`; Protocol: JSON-RPC over Unix socket
- Gate — behavior: `cargo test -p compass` - configured daemon, watch,
  and incremental update smoke gate
- Gate: `cargo test -p compass`
- Source: `crates/compass/src/interfaces/daemon/argus_daemon.rs`,
  `crates/compass/src/application/analysis/request_handler.rs`,
  `crates/compass/src/domain/incremental/update_manager.rs`,
  `crates/compass/src/domain/incremental/dirty_file_tracker.rs`,
  `crates/compass/src/infrastructure/watch_bridge/bridge.rs`,
  `crates/compass/src/infrastructure/watch/file_watcher.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Argus daemon protocol and request handling contract | epic | - | `cargo test -p compass`; crates/compass/src/interfaces/daemon/argus_daemon.rs; crates/compass/src/application/analysis/request_handler.rs; crates/compass/src/application/daemon/protocol.rs |
| Watch bridge and incremental dirty-file contract | epic | - | `cargo test -p compass`; crates/compass/src/domain/incremental/update_manager.rs; crates/compass/src/domain/incremental/dirty_file_tracker.rs; crates/compass/src/infrastructure/watch_bridge/bridge.rs; crates/compass/src/infrastructure/watch/file_watcher.rs |
