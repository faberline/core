# compass

compass is the code-intelligence context. It parses source in 17 languages with
tree-sitter and models what the parse reveals: checker diagnostics, callable
outlines, symbols and scopes, types propagated across imports, program
dependence graphs, semantic search and refactorings. It also validates specs
such as state machines and generates code from them. An LSP server and a
JSON-RPC daemon serve these analyses; the daemon re-analyzes incrementally as
files change. No other core crate depends on compass. Downstream repos guard and
meter use it: guard to lint and scan for SQL injection, meter to list the
functions it instruments.

**Form:** layered · **Depends on:** — · **Crate:** [`crates/compass`](../../crates/compass)

## Model

- **Syntax** — `Language` names the 17 languages, from Python and Rust to
  Dockerfile, Mermaid, SQL and GraphQL; `Language::from_path` detects a file's
  language from its path. A `SourceParser` parses source into a `ParsedFile`:
  source, tree-sitter tree, language, whether the parse had errors, and whether
  it is line-based. A node's `Range` comes from the `NodeRange` trait
  (`node.to_range()`).
- **Diagnostics** — `Diagnostic` is one finding: a `Range` of two `Position`s,
  a `DiagnosticSeverity` (Error, Warning, Information, Hint, numbered 1–4 as in
  LSP), a `RuleCode` (the rule's code, such as `PY001`, serialized as a bare
  string), a `DiagnosticCategory` (Syntax, Type, Names, Logic,
  Security, Style, Custom), a message, and `QuickFix`es made of `TextEdit`s.
- **Checking** — a `Checker` lints one language; `CheckerRegistry` holds them.
  `LintConfig` selects languages, path exclusions and a minimum severity
  (private fields: start from `Default` and use the `with_*` builders);
  `FileResult` is one file's diagnostics. `detect_sql_injection` is a
  standalone security check over source text. `CustomLintEngine` runs
  user-defined rules; a regex rule whose pattern does not compile is kept as
  a `RejectedRule` (`rejected_rules()`), and the file loader logs it.
- **Outline** — `FunctionDef` is one callable definition (name, `FunctionKind`,
  first and last line); `outline` lists them for Rust, Python, TypeScript,
  JavaScript and Go.
- **Semantic model** — `SymbolTable`, scopes from `ScopeAnalyzer`, and for
  Python a `ProgramDependenceGraph` for slicing, impact, taint and dead code.
- **Types** — `DeepTypeInferencer` infers `Type`s within each file;
  `PropagationPipeline` carries them across `ImportGraph` edges into a
  `PropagationResult` (propagated types, cycles, stats). `SemanticModel` is the
  analysis the disk cache persists.
- **Search and refactoring** — `SemanticSearchEngine` runs semantic code
  search; it indexes a parsed file's call graph and docstrings
  (`build_call_graph_parsed`, `extract_docstrings_parsed`). A `RefactorKind`
  (rename, extract, inline, move definition, change signature) is applied by
  `RefactoringEngine`, which parses through a `SourceParser`
  (`RefactoringEngine::with_parser`).
- **Incremental analysis** — `DirtyFileTracker` records each file's
  `FileChangeKind`; `DependencyGraph` records imports;
  `IncrementalUpdateManager` combines them into the files to re-analyze.
- **Serving** — the daemon's JSON-RPC protocol (`Request`, `Response`,
  `RpcError` and the per-method params), `DaemonConfig` and the watch
  `BridgeEvent` are domain values. In application, `RequestHandler` answers one
  scope's requests over a parser and an `AnalysisCache`, and `DaemonService`
  routes requests to the handler of the longest matching scope and starts the
  background watch through an injected start function. The LSP server's
  documents, analyses, hover, definition, references, completions and
  refactorings are use cases of an application editor session; the server
  itself only translates LSP messages. The agent and text reporters render the
  check results together with application views of the symbol tables and the
  import graph.
- **Specs and generation** — `SpecIR` is a data model, REST API, event API,
  state machine or control flow spec. `StateMachineValidator` checks a
  `StateMachineDef` into a `ValidationResult`; `MermaidPlusGenerator` renders
  it. `GeneratorRegistry` picks a `CodeGenerator` to emit `GeneratedCode`.
- **Errors** — `ArgusError` and `Result<T>`. The legacy names Argus and Lens
  are kept (ADR compass #5).

## Ports

- `SourceParser` — parses source into a `ParsedFile`, or a line-based one;
  `MultiParser` (the tree-sitter grammars, `infrastructure/syntax`) implements
  it.
- `SourceWalker` (crate-private) — the files under a path and their source;
  `FsSourceWalker` implements it.
- `AnalysisCache` (crate-private) — loads, stores and invalidates a file's
  cached semantic model; the daemon's `DiskCache` implements it. The public
  `type_inference::AnalysisCache` is an unrelated module-cache struct.
- `Checker` — lints one `ParsedFile`; one implementation per `Language`
  (`PythonChecker`, `RustChecker`, …, `YamlDispatcher` for YAML).
- `CodeGenerator` — emits code for a serialized spec. The contract is in
  domain; ten stack generators in infrastructure, such as `AxumGenerator`,
  `SqlxGenerator` and `SerdeGenerator`, implement it.
- `FrameworkTypeProvider` — types of framework attributes and methods; Django,
  FastAPI and Pydantic providers implement it.

The composition root (`src/app.rs` and `src/app/`) builds these adapters and
keeps the public entry points that need them: `check_paths`,
`check_paths_with_propagation`, `outline`, `RequestHandler::new`,
`ArgusDaemon::new`, `ArgusServer::new`, `run_server`, `run_server_tcp`,
`RefactoringEngine::new`, the source-text `SemanticSearchEngine` methods,
`AgentOutputBuilder::build` and `Reporter::generate_agent`. Their paths and
signatures are unchanged.

## Invariants

- `CheckerRegistry` holds exactly one checker per `Language`.
- `check_paths` skips files whose language is undetected or disabled. If the
  parser yields no tree, Dockerfile, Markdown, MDX and Mermaid fall back to a
  line-based `ParsedFile`; other languages are skipped.
- `Position` is 0-indexed and `Range::contains` includes both ends;
  `FunctionDef` lines are 1-based and sorted by start line.
- `DirtyFileTracker::mark_dirty` keeps the strongest change: Deleted over
  Modified over Created. `drain` consumes and clears the set in one step;
  `drain_dirty_files` adds every transitive importer and sorts the result.
- `PropagationPipeline::run` visits dependencies before dependents, prefers
  `.pyi` stubs over `.py` sources, and marks the members of import cycles.
- `StateMachineValidator` reports a missing initial state as an error;
  `MISSING_COMPOUND_INITIAL` is a warning, or an error in strict mode. Any
  error sets `ValidationResult.valid` to false.
- `GeneratorRegistry` delegates to the first registered generator whose
  `can_generate` accepts the spec, else returns `GenError::UnsupportedFeature`.

## Published language

No other core context depends on compass. guard imports `check_paths` and
`LintConfig` (by the `checker::` path, which P2 deleted),
`diagnostic::{DiagnosticCategory, DiagnosticSeverity}`,
`lint::detect_sql_injection` and `syntax::Language`; meter imports `outline`,
`FunctionKind` and `syntax::Language`. The daemon's JSON-RPC 2.0 methods
(`check`, `type_at`, `symbols`, `diagnostics`, `hover`, `definition`,
`references`, `pdg`, `slice`, `impact`, `taint` and housekeeping) and the LSP
server use `Diagnostic` and `Range` as wire format. Sixteen public modules
keep their paths because they hold names the root does not re-export
(`src/api/`), with nested paths such as `server::incremental`, which a doctest
imports. P2 deleted the old modules `checker`, `outline` and `watch`: every
name in them is at the crate root, and `compass::outline` is now only the
function.

## Exceptions and debts

- **Checker exceptions (all long-term):**
  - B2 `tree_sitter` in 48 domain files (E1): the syntax model is
    tree-sitter's. `ParsedFile` wraps a `tree_sitter::Tree`, and the lint
    checkers, semantic visitors, CFG builder, type inference, type checker,
    semantic search and refactoring AST cache walk `tree_sitter::Node`
    directly on the hot path. Only the core `tree-sitter` crate is named in
    domain; the 13 grammar crates stay in infrastructure.
  - B2 `serde_yaml` (Mermaid+ frontmatter) and `toml` (the TM001 syntax
    check): pure in-memory data-format codecs, like `serde_json`.
  - B3 `interfaces->domain` (E2): `lsp/argus_server.rs` converts
    `Diagnostic`, `DiagnosticSeverity`, `Range` and `Position` to and from
    `lsp_types`; `output/agent.rs` and `output/reporter.rs` render
    `FileResult`, `Diagnostic`, `DiagnosticSeverity` and `Range`. These are
    the wire value objects; everything else reaches interfaces through
    application.
  - Serde derives on domain values are allowed by policy and need no
    exception.
- **Not checker findings:** the Markdown relative-link check (MD011) still
  probes the file system with `Path::exists` from the domain; it needs a
  file-system port. `SchemaRegistry::global` and the frontmatter validators
  are `OnceLock` singletons in `infrastructure/schema_validation`.
- **Debts:** `CodeGenerator` takes `serde_json::Value` to avoid a
  circular crate dependency. `Position`, `Range`, `TextEdit`, `Diagnostic`,
  `QuickFix` and the `SpecIR` structs keep public fields by design (wire value
  objects and spec IR). Other aggregates with public fields that code outside
  their module writes: `ModuleNode`, `ParsedFile`, `RustStruct` and
  `RustEnum`, `SearchResult`, `FileAnalysis`, and `MutableNode` with
  `NodeId`. Bare identities: `PathBuf` as file identity, `NodeId` with a
  public `.0`, and the symbol table's `SymbolId` with a public `.0`, unrelated
  to the semantic model's `SymbolId` (whose value is private, like
  `ScopeId`'s).
