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
  Dockerfile, Mermaid, SQL and GraphQL. `MultiParser` detects a file's language
  from its path and parses it into a `ParsedFile`: source, tree-sitter tree,
  language, whether the parse had errors, and whether it is line-based.
- **Diagnostics** — `Diagnostic` is one finding: a `Range` of two `Position`s,
  a `DiagnosticSeverity` (Error, Warning, Information, Hint, numbered 1–4 as in
  LSP), a rule code, a `DiagnosticCategory` (Syntax, Type, Names, Logic,
  Security, Style, Custom), a message, and `QuickFix`es made of `TextEdit`s.
- **Checking** — a `Checker` lints one language; `CheckerRegistry` holds them.
  `LintConfig` selects languages, path exclusions and a minimum severity;
  `FileResult` is one file's diagnostics. `detect_sql_injection` is a
  standalone security check over source text.
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
  search. A `RefactorKind` (rename, extract, inline, move definition, change
  signature) is applied by `RefactoringEngine` or `RefactoringRegistry`.
- **Incremental analysis** — `DirtyFileTracker` records each file's
  `FileChangeKind`; `DependencyGraph` records imports;
  `IncrementalUpdateManager` combines them into the files to re-analyze.
- **Specs and generation** — `SpecIR` is a data model, REST API, event API,
  state machine or control flow spec. `StateMachineValidator` checks a
  `StateMachineDef` into a `ValidationResult`; `MermaidPlusGenerator` renders
  it. `GeneratorRegistry` picks a `CodeGenerator` to emit `GeneratedCode`.
- **Errors** — `ArgusError` and `Result<T>`. The legacy names Argus and Lens
  stay unchanged in P1.

## Ports

- `Checker` — lints one `ParsedFile`; one implementation per `Language`
  (`PythonChecker`, `RustChecker`, …, `YamlDispatcher` for YAML).
- `CodeGenerator` — emits code for a serialized spec; implemented by ten stack
  generators such as `AxumGenerator`, `SqlxGenerator` and `SerdeGenerator`.
- `RefactoringOp` — applies one refactor request; implemented by the rename,
  extract, inline, move-definition and signature engines.
- `FrameworkTypeProvider` — types of framework attributes and methods; Django,
  FastAPI and Pydantic providers implement it.

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

No other core context depends on compass. guard imports `checker::{check_paths,
LintConfig}`, `diagnostic::{DiagnosticCategory, DiagnosticSeverity}`,
`lint::detect_sql_injection` and `syntax::Language`; meter imports `outline`,
`FunctionKind` and `syntax::Language`. The daemon's JSON-RPC 2.0 methods
(`check`, `type_at`, `symbols`, `diagnostics`, `hover`, `definition`,
`references`, `pdg`, `slice`, `impact`, `taint` and housekeeping) and the LSP
server use `Diagnostic` and `Range` as wire format. P1 compat facades keep all
22 old public modules, every crate-root re-export, and nested paths such as
`server::incremental`, which a doctest imports.

## Exceptions and debts

- **Checker exceptions (P1):**
  - B2 `tree_sitter` in 72 domain files: `ParsedFile` holds a tree,
    `ArgusError` wraps its language error, and the checkers, semantic visitors,
    type inference, search and refactoring walk nodes directly. This is a
    long-term exception (E1); the grammar crates stay in
    `infrastructure/syntax`.
  - B2 `regex_lite` in custom lint rules, the config glob matcher, the import
    extractors and TypeScript template-literal matching (E6); P2 switches them
    to `regex`.
  - B2 `serde_yaml` (Mermaid+ frontmatter), `toml` (the TM001 syntax check),
    `tracing` (rejected custom rules), and `anyhow` with `bincode`
    (`SearchIndex` bytes).
  - B2 `Instant::now` in `DirtyFileTracker` and the six search modes; P2 adds
    a `Clock` port.
  - B3 `application->infrastructure` and `domain->infrastructure`: use cases
    and domain engines build a `MultiParser`, load stubs, resolve imports, or
    hold the disk cache themselves; P2 adds parser, stub, cache and path
    ports. `GeneratorRegistry` dispatches over the `CodeGenerator` trait in
    infrastructure (E5).
  - B3 `infrastructure->application`: `DaemonClient` uses `DaemonConfig` and
    the protocol types.
  - B3 `interfaces->infrastructure`: the daemon starts the watch bridge and
    scope discovery, and the LSP server builds its parser and stubs.
  - B3 `interfaces->domain`: the LSP server calls domain services directly;
    P2 routes those calls through application. The reporters and LSP types
    read `Diagnostic`, `Range` and `FileResult`, which are the wire format;
    that stays with a long-term reason.
  - Serde derives on domain values are allowed by policy and need no
    exception.
- **Not checker findings:** the Markdown relative-link check (MD011) probes
  the file system with `Path::exists` from the domain; P2 adds a file-system
  port for it. The import-path probe (`resolve_import_path`) now lives in
  infrastructure, and the clock reads in propagation, background analysis and
  incremental analysis sit in application or infrastructure, where policy
  allows them.
- **Tracked for P2:** `CodeGenerator` takes `serde_json::Value` to avoid a
  circular crate dependency. Public fields on `Position`, `Range`, `TextEdit`,
  `Diagnostic`, `LintConfig` and the `SpecIR` structs. Bare identities:
  `PathBuf` as file identity, the rule code as a `String`, id newtypes with a
  public `.0` (`ScopeId`, `NodeId`), and two unrelated `SymbolId` types.
  `SchemaRegistry::global` is a `OnceLock` singleton. `Range::from_node` takes
  a tree-sitter node: an inherent method in P1, an extension trait in P2. Dead
  or duplicated code (ADR D7): `search::SearchEngine` duplicates
  `SemanticSearchEngine` and has no callers; `RefactoringRegistry` and
  `RefactoringEngine` are two refactoring engines; an unused
  `type_inference::CodeGenerator` struct shares the port's name; `format`,
  `lint::{autofix, custom, embedded_markdown}`, `semantic::types` and
  `GoImportGraph` are unused; the `type_inference` Rust and TypeScript modules
  have no outside users; and dependencies such as `tera`, `heck`, `indexmap`
  and `crossterm` are never used in source.
