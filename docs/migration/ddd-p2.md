# Migrating to DDD P2

P2 (branch `refactor/ddd-p2`, stacked on
[P1](ddd-p1.md)) finishes the layered refactor described in
[architecture](../architecture.md) and
[ADR 0001](../adr/0001-standard-layout-and-ddd.md). Unlike P1, it changes the
public API. It:

- fixes the architecture exceptions that P1 recorded with a planned fix
- removes the compatibility facades that only repeated root names, and keeps
  the other module paths as public API modules
- deletes unused public items (ADR D7)
- makes the public fields of configuration and input types private, behind
  constructors and getters (ADR D2)
- replaces `anyhow` in the traits that downstream code implements with each
  crate's own error type (ADR D4)
- completes the planned newtype ids, versions, cursors and generations

**When to change your code.** Core has no release tag that contains P2 yet.
Nothing breaks until your repo moves its core pin to a revision that
includes P2. Each downstream repo's maintainer gets a list of the lines that
break and how to fix each one. This guide explains the changes behind those
lists.

## What does not change

- **Persisted and wire formats.** Before each change to a serialized type,
  P2 added a golden test that pins its exact JSON or bytes, and every golden
  test passes unchanged after it. This covers catalog pages, archive
  receipts, projection checkpoints and state files, claim tokens, text index
  snapshots, the surface snapshot, CRD conditions and specs, and the
  generated JSON and OpenAPI schemas.
- **Error text.** Where an error type changed, its Display output is the
  same text, so logs and assertions on messages still match. The one
  exception is service-backup's snapshot fetch, described under its
  **Behaviour**. Debug output can differ: `{:?}` on the new error type
  itself, and so the panic message of `.unwrap()` or `.expect()` on its
  `Result`, prints the derived variant form instead of anyhow's "message
  and Caused by" form. Conversion with `?` still uses anyhow's error-chain
  format. Debug structure, downcasts and backtraces can change; see the
  error notes for each crate.
- **Behaviour.** Behaviour is unchanged unless a crate section below says
  otherwise under **Behaviour**.
- **Feature flags.** No crate added or removed a feature.

## Common patterns

**Private fields (ADR D2).**

| Before | After |
|---|---|
| `T { a, b }` | `T::new(a, b)`, or `T::try_new(a, b)?` when the type validates |
| `T { a, ..Default::default() }` | `T::default().with_a(a)` |
| `t.a` (a `Copy` field) | `t.a()` |
| `&t.a`, `t.a.as_deref()` | `t.a()`, which borrows (`&str`, `&[u8]`, `Option<&str>`, …) |
| `t.a` moved out | `t.into_parts()`, where the type has it |

Output types that only the library builds keep public fields, as do wire
DTOs. Examples are reports, snapshots, receipts and error envelopes. Each
section names the ones you might expect to change.

**Error types on traits you implement (ADR D4).**
- A trait method that returned `anyhow::Result<T>` now returns
  `Result<T, CrateError>`.
- Each such error type has a transparent `Other` variant for your errors.
  `CrateError::other(e)` accepts anything that converts into
  `Box<dyn std::error::Error + Send + Sync>`, including `anyhow::Error`.
  Display and the source chain are kept.
- In an implementation whose body uses anyhow, wrap the result:
  `body().map_err(CrateError::other)`.
- Code that calls these methods from an `anyhow` function keeps using `?`.
- A direct `anyhow::Error::downcast_ref::<YourError>()` is not generally
  preserved through a new wrapper. Inspect `chain()` or the typed `Other`
  variant. The raft-runtime section names the host paths that preserve it.

**Removed facade modules.** When a module path such as
`service_http::config::HttpConfig` is gone, the same item is at the crate
root (`service_http::HttpConfig`). The items are identical, so only the
`use` line changes.

**Newtype values (W5).** A named type distinguishes one identity or position
from another. Wrap the raw value with `Type::new(value)`. Use `get()` for
numbers and `as_str()` for strings at product boundaries. Serde still emits
the original primitive, not an object with a new field.

## Downstream checks

The [downstream report](ddd-p2-downstream.md) lists the inspected revisions,
compiler diagnostics and the migration section for each repo. Source paths
in that report refer to those fixed local snapshots. Downstream working trees
are not changed by core. [The source-path table](ddd-p2-paths.md) maps P1 files
to their P2 locations.

The [validation record](ddd-p2-validation.md) gives the full test results,
test count changes and the public API inventory for all 30 crates.


## claim-token

**Private fields and identity types.** `Scope::new` now takes `InputKey`,
`ResultKey` and `ExpiryUnixSeconds`. The key types prevent a caller from
swapping read and write access. `r()` and `w()` return references to their
key types; call `as_str()` for the raw key. `exp()` returns `ExpiryUnixSeconds`;
call `get()` for the number. `verify(secret, token, now)` still takes Unix
seconds as a `u64` and keeps its inclusive expiry check.

```rust
use claim_token::{ExpiryUnixSeconds, InputKey, ResultKey, Scope};
let scope = Scope::new(InputKey::new("docs/*"), ResultKey::new(""), ExpiryUnixSeconds::new(exp));
if scope.w().as_str() == id { /* existing check */ }
```

- Affects: keep and loom.
- The scope JSON and signed token bytes are unchanged, including expiry 0
  and `u64::MAX`. Existing golden fixtures and deadline tests pin them.

## cli-std

**Paths.**
- No path changes.
- The public entry points keep their paths and signatures:
  `issue::{create, comment, search, view}`, `upgrade::run`, and
  `connect::{resolve_token, resolve_cr_tokens_secret}`.

**Removed.** These three functions had no callers:
- `connect::kubectl_get_json`: run `kubectl get … -o json` yourself.
- `connect::secret_data_bytes`: decode `data.<key>` with `base64` yourself.
- `chainable::assert_command_chainable`: pass the command's stdout to
  `chainable::assert_chainable`.

**Private fields.** Each type below has a constructor, and each field has a
getter of the same name.

| Type | Build with |
|---|---|
| `ToolInfo` | `ToolInfo::new(project, repo, target, version, git_sha, built_at)`, a `const fn` |
| `llm::Topic` | `Topic::new(id, summary, body)`, a `const fn` |
| `llm::SectionedTopic` | `SectionedTopic::new(id, summary, sections)`, a `const fn` |
| `issue::CreateOptions` (also `report_issue::Options`) | `CreateOptions::new(title)`, then `with_message`, `with_url`, `with_repo`, `with_label`, `with_dry_run`, `with_yes` |
| `issue::CommentOptions` | `CommentOptions::try_new(number)?`, then `with_message`, `with_repo`, `with_dry_run`, `with_yes` |
| `issue::SearchOptions` | `SearchOptions::default()`, then `with_query`, `with_state`, `with_limit` |
| `upgrade::Options` | `Options::default()`, then `with_check`, `with_tag`, `with_force`, `with_yes` |

- The getters of the three `const fn` types are also `const fn`, so
  `TOOL.version()` works in a const context.
- `with_message`, `with_url`, `with_repo`, `with_query` and `with_tag`
  take `impl Into<Option<String>>`, so a `String` and an `Option<String>`
  both work.
- A field left out of the literal before is the default now, so drop
  `url: None`, `repo: None` and `force: false`.

```rust
// before
const TOOL: cli_std::ToolInfo = cli_std::ToolInfo {
    project: "lumen",
    repo: "faberline/lumen",
    target: env!("LUMEN_TARGET"),
    version: env!("CARGO_PKG_VERSION"),
    git_sha: env!("LUMEN_GIT_SHA"),
    built_at: env!("LUMEN_BUILT_AT"),
};
const TOPICS: &[cli_std::llm::Topic] = &[cli_std::llm::Topic {
    id: "workflow",
    summary: "how it works",
    body: WORKFLOW_MD,
}];
let opts = cli_std::issue::CommentOptions {
    number: args.number,
    message,
    repo: None,
    dry_run: args.dry_run,
    yes: true,
};
println!("{}", TOOL.version);

// after
const TOOL: cli_std::ToolInfo = cli_std::ToolInfo::new(
    "lumen",
    "faberline/lumen",
    env!("LUMEN_TARGET"),
    env!("CARGO_PKG_VERSION"),
    env!("LUMEN_GIT_SHA"),
    env!("LUMEN_BUILT_AT"),
);
const TOPICS: &[cli_std::llm::Topic] =
    &[cli_std::llm::Topic::new("workflow", "how it works", WORKFLOW_MD)];
let opts = cli_std::issue::CommentOptions::try_new(args.number)?
    .with_message(message)
    .with_dry_run(args.dry_run)
    .with_yes(true);
println!("{}", TOOL.version());
```

**Issue number check.**
- `CommentOptions` no longer implements `Default`, because its default was
  issue number 0.
- `try_new(0)` returns the new `cli_std::IssueNumberError::Zero`, with the
  same message as before: "issue number must be positive". The check used to
  run inside `issue::comment`; it now runs in `try_new`. Either way it runs
  before anything is printed or sent.

**Who is affected.**
- Every repo with a standard CLI builds `ToolInfo`, `upgrade::Options` and
  the issue options by literal: beam, cap, courier, defer, jet, keep, loom,
  lumen, mamba, mesh, pgpool, relay, sift, tape and vat.
- `llm::Topic` literals are in most of the same repos. lumen reads `.body`
  on core's `llm::topic()` values; use `.body()`.
- `SectionedTopic` is used only by tape.
- Each repo's notice lists its lines.

## compass

**Rule codes: `RuleCode`.** `Diagnostic.code` was a `String` and is now a
`compass::RuleCode`. `RuleCode` is `#[serde(transparent)]`, so the JSON and
cache bytes of a `Diagnostic` are unchanged.
- `diag.code == "E001"` still compiles, and so does `format!("{}", diag.code)`.
- Where a `&str` is expected, write `diag.code.as_str()`; `&diag.code` no
  longer converts.
- Where a `String` is expected, write `diag.code.to_string()`, or
  `into_string()` to move it out.
- `Diagnostic::new`, `error` and `warning` take `impl Into<RuleCode>`, so a
  `&str` or `String` argument still works.

```rust
// before
if !include(diagnostic.category, &diagnostic.code) { continue; }
let rule: String = diagnostic.code.clone();
// after
if !include(diagnostic.category, diagnostic.code.as_str()) { continue; }
let rule: String = diagnostic.code.to_string();
```

**Private fields.**
- `LintConfig`: build it from `LintConfig::default()` with `with_languages`,
  `with_exclude_patterns` and `with_min_severity`. Read it with
  `languages()`, `exclude_patterns()` and `min_severity()`.
- `compass::type_inference::{ScopeId, SymbolId}` (the semantic-model ids):
  `ScopeId(n)` becomes `ScopeId::new(n)` and `id.0` becomes `id.get()`.
  `compass::semantic::SymbolId` is a different type and is unchanged.
- `Diagnostic`, `Position`, `Range`, `TextEdit` and `QuickFix` keep their
  public fields.

```rust
// before
let config = LintConfig {
    languages: options.languages,
    exclude_patterns: options.exclude_patterns,
    min_severity: DiagnosticSeverity::Hint,
};
// after
let config = LintConfig::default()
    .with_languages(options.languages)
    .with_exclude_patterns(options.exclude_patterns)
    .with_min_severity(DiagnosticSeverity::Hint);
```

**Node ranges.** `Range::from_node(&node)` is gone, because the domain type
`Range` no longer names `tree_sitter`. Import the new trait and call
`node.to_range()`:

```rust
use compass::NodeRange;
let range = node.to_range();
```

**Removed.** These had no callers inside or outside the workspace.

| Removed | Use instead |
|---|---|
| `compass::search::*` (`SearchEngine`, `SearchIndex`, …) | `compass::type_inference::SemanticSearchEngine` |
| `compass::refactoring::*` (`RefactoringRegistry`, the five engines) | `compass::type_inference::RefactoringEngine` |
| `compass::format::*` | nothing; call the formatter directly |
| `compass::lint::autofix::*`, `compass::lint::embedded_markdown::*` | nothing |
| `compass::lint::markdown::{MarkdownSymbol, MarkdownSymbolExtractor}` | `compass::semantic::SymbolTableBuilder` |
| `compass::semantic::types::*` (Go types) | nothing |
| `compass::type_inference::{rust_*, ts_*, refactoring_multilang, semantic_search_rust}` and the `Ts*` types, `is_assignable_to`, `Visibility`, `MappedTypeModifier`, `TemplatePart` | nothing |
| `compass::type_inference::{CodeGenerator, CodeGenKind, CodeGenOptions, CodeGenRequest, CodeGenResult, DocstringStyle, TestFramework}` | the `compass::CodeGenerator` trait, which is unchanged |
| `ArgusError::TreeSitterLanguage` and `From<tree_sitter::LanguageError>` | map the error at the `set_language` call |
| the `compass::type_inference::propagation::` path | the same names at `compass::type_inference::` |

**Added.**
- `compass::RejectedRule` and `CustomLintEngine::rejected_rules()`.
- `compass::SourceParser` and `Language::from_path`.
- `SemanticSearchEngine::{build_call_graph_parsed, extract_docstrings_parsed,
  index_symbol_table_parsed}` and `RefactoringEngine::with_parser`.

**Behaviour.**
- `CustomLintEngine::from_rules_file` no longer logs a rule whose pattern
  does not compile. It returns the rule in `rejected_rules()` instead.
  `load_from_path` and `load_from_workspace` still log each one at WARN
  with the same message.
- `RequestHandler::new_with_scope` accepts `extra_search_paths` and
  ignores it. The paths only fed a field that nothing read.
- Custom rule patterns are now compiled by `regex` instead of `regex-lite`,
  through a translator that keeps regex-lite's ASCII meaning of `\w`, `\s`,
  `\b` and `(?i)`. Matches and rejection messages are unchanged.

- Affects: guard (`src/scan.rs`: the `LintConfig` literal and three
  `&diagnostic.code` arguments plus one `.clone()` into a `String`). meter
  is not affected.
- Unchanged: `check_paths`, `check_paths_with_propagation`, `outline`,
  `RequestHandler::new`, `ArgusDaemon::new`, `ArgusServer::new`,
  `run_server` and `run_server_tcp` keep their paths and signatures. The
  diagnostic JSON, the semantic-model cache and the daemon protocol bytes
  are pinned by golden tests.

**Modules removed.**
- `compass::checker`, `compass::outline` and `compass::watch` were facades
  over names the crate root already exports.
- Change `compass::checker::X`, `compass::outline::X` and
  `compass::watch::X` to `compass::X`; each is the same item.
- The function `compass::outline(..)` is still at the root. Only the module
  of the same name is gone.
- Affects: guard (`compass::checker::{check_paths, LintConfig}`). meter calls
  the root function `compass::outline` and is not affected.

## index-text

**Private fields and identity types.** `TextDocument::new` takes
`DocumentId` and `DocumentVersion`. `external_id()` returns `&DocumentId`;
`version()` returns `DocumentVersion`. `fields()` and `with_field` keep their
signatures. `TextHit` keeps public fields, but its id and version are typed.
`TextIndex::delete` takes `&DocumentId` and `Option<DocumentVersion>`.
Snapshot tombstones are `BTreeMap<DocumentId, DocumentVersion>`.

```rust
use index_text::{DocumentId, DocumentVersion, TextDocument};
let doc = TextDocument::new(DocumentId::new(id), DocumentVersion::new(version));
let id_text = doc.external_id().as_str();
let version_number = doc.version().get();
```

- Affects: sift's text index adapter, query results and snapshot code.
- Document and snapshot JSON keep the same bytes. Old snapshots decode.
  A maximum-version tombstone still blocks stale replay after restore.

## metrics-prometheus

**Private fields.** `Sample`, `Label`, `Bucket`, `SampleGroup`,
`LabeledSample` and `Latency` keep their constructors, and every field has a
getter of the same name. The getters of the first four are `const fn`, so
const tables still compile.

| Type | Build with | Getters |
|---|---|---|
| `Sample` | `Sample::new(..)` (unchanged) | `name()`, `kind()`, `help()`, `value()` |
| `Label` | `Label::new(name, value)` | `name()`, `value()` |
| `Bucket` | `Bucket::new(le, max)` | `le()`, `max()` |
| `SampleGroup` | `SampleGroup::new(..)` | `name()`, `kind()`, `help()`, `samples()` |
| `LabeledSample` | `LabeledSample::new(labels, value)` | `labels()`, `value()` |
| `Latency` | `Latency::new()` / `Default` | `sum()`, `count()`, both `&Counter` |

```rust
// before
let b = Bucket { le: "0.5", max: 500 };
let total = latency.sum.get();
// after
let b = Bucket::new("0.5", 500);
let total = latency.sum().get();
```

- Affects: tape and relay (`Latency` `.sum` / `.count` reads). Every repo
  already builds these types with `new`.
- The rendered exposition text is unchanged.

## metrics-remote-write

**Removed.** `ValidatedWrite::request()`. Use `into_inner()`, which returns
the `WriteRequest` by value. No downstream use was found.

## openapi-codegen

**Private fields: `GenOptions`.**
- Build it with `GenOptions::new(lang, spec_path, out_dir, client_name)`.
- `new` starts with no target, `HttpClient::Fetch`, types and client on,
  and hooks on only for `Lang::Ts`. Every downstream literal found used
  these values, apart from the target and the HTTP client.
- To change them, add `.with_target(target)`, `.with_http_client(c)`,
  `.with_emit_types(b)`, `.with_emit_client(b)` or `.with_emit_hooks(b)`.
- Each field has a getter of the same name. `spec_path()` and `out_dir()`
  return `&Path`, and `client_name()` returns `&str`.

```rust
// before
let opts = GenOptions {
    lang,
    target: Some(target),
    spec_path: PathBuf::new(),
    out_dir: args.out.clone(),
    client_name: "createClient".to_string(),
    http_client: match args.http { GenHttp::Fetch => HttpClient::Fetch, GenHttp::Axios => HttpClient::Axios },
    emit_types: true,
    emit_client: true,
    emit_hooks: matches!(lang, Lang::Ts),
};
// after
let opts = GenOptions::new(lang, PathBuf::new(), args.out.clone(), "createClient")
    .with_target(target)
    .with_http_client(match args.http { GenHttp::Fetch => HttpClient::Fetch, GenHttp::Axios => HttpClient::Axios });
```

Drop `.with_target(..)` when the literal had `target: None`, and drop
`.with_http_client(..)` when it had `HttpClient::Fetch`.

**Errors.** These functions returned `anyhow` and now return the crate's
own error types, all at the crate root. Code that uses `?` in an `anyhow`
function compiles unchanged.

| Function | Error type |
|---|---|
| `emit::{ts, py, rust}::generate*` | `SpecParseError` |
| `FileBearerAuth::new` | `FileBearerAuthError` |
| `TargetProfile::from_id`, `str::parse::<TargetProfile>` | `UnknownTargetProfile`, with an `id()` getter |
| `TargetPolicy::resolve` | `TargetPolicyError` |

The root `generate*` functions and `TargetPolicy::from_toml` still return
`anyhow`, and `run` still returns an exit code. `SpecParseError` is not `UnwindSafe` or `RefUnwindSafe`,
which `anyhow::Error` was; this matters only if you move it into
`catch_unwind` without `AssertUnwindSafe`.

**Removed.** `TargetProfile::default_for(lang)` had no callers. Name the
profile instead: `TargetProfile::TypeScript(TypeScriptTarget::Ts50)`,
`TargetProfile::Python(PythonTarget::Py311)` or
`TargetProfile::Rust(RustTarget::Rust2021)`. Or call `policy.for_lang(lang)`
on a loaded `TargetPolicy`.

- Affects: `GenOptions` literals in defer, keep, loom, lumen, relay, sift
  and tape. jet has its own `GenOptions` and is not affected.
- Unchanged: `openapi_codegen::run` keeps its path, signature, output text
  and exit codes. The generated TypeScript, Python and Rust files and the
  manifest are the same bytes; golden tests pin them.

**Module removed.** `openapi_codegen::target` was a facade over root names.
Change `openapi_codegen::target::X` to `openapi_codegen::X`. No downstream
use was found. `emit`, `ir` and `llm` stay.

## peer-tls

**Private fields.** `PeerTlsConfig`:
- Build it with `PeerTlsConfig::new(cert, key, ca, required)`. The three
  paths take `impl Into<PathBuf>`. `new` does not check the files, as before.
- Read it with `cert()`, `key()` and `ca()`, which return `&Path`, and
  `required()`, which returns `bool`.
- There is no setter. To change one path, build a new config from the old
  one's getters.

```rust
// before
PeerTlsConfig { cert, key, ca, required: true }
if cfg.required { … }
// after
PeerTlsConfig::new(cert, key, ca, true)
if cfg.required() { … }
```

- Affects: sift, defer, tape, relay and lumen (test literals, and the
  `From` impls in lumen `src/tls.rs` and relay `src/peer_tls.rs`).

## raft-core

**Private membership fields.** Replace `Membership { voters, learners }`
with `Membership::new(voters, learners)`. Read `voters()` and `learners()`;
use `into_parts()` to move the vectors out. Each vector contains `NodeId`.

**Newtypes.** `NodeId`, `Term` and `Index` replace `u64` aliases. Construct
each with `new(number)`; read with `get()`. There is no implicit conversion.
They keep bare-number JSON, map keys, Debug and Display. `Term::next` and
`Index::{next,prev,saturating_add,saturating_sub,checked_add}` cover the host's
index operations. Product counters, atomics and application wire fields can
stay `u64`; convert at the Raft boundary.

```rust
use raft_core::{Index, Membership, NodeId, Term};
let members = Membership::new(vec![NodeId::new(0)], Vec::new());
let applied = Index::new(raw_applied);
let term = Term::new(raw_term);
// For an AtomicU64 or a tracing number: applied.get(), term.get().
```

**New driver ports.** `RaftStorage` persists the hard state and pins committed
command bytes through `PinnedCommand` and `CommandLease`. `RaftDelivery`
delivers peer messages and installs snapshots. They use std futures and
`io::Result`; raft-runtime provides the file and HTTP adapters.

**Removed items.** `RaftTransport` and `RaftNode::learner_replication_gap`.
Drivers already drain `take_outgoing`. For a learner gap, use
`commit_index().get().saturating_sub(learner_matched(peer)?.get())`.

- Affects: defer, keep, loom, lumen, relay, sift and tape.
- JSON, membership and conf-state encodings keep their golden bytes.

## raft-runtime

**Port errors.** Every `RaftStateMachine` method, plus
`SnapshotPreparation::capture_at` and `PreparedSnapshot::write_to`, returns
`Result<_, StateMachineError>`. `MembershipPolicy::validate` returns
`Result<(), MembershipError>`. `StateMachineError::PrefixUnavailable` keeps
the old default snapshot refusal text. Both errors have transparent `Other`
variants and `other(e)` constructors.

For an existing anyhow implementation, use a helper that returns anyhow and
convert with `?`, `error.into()` or `map_err(StateMachineError::from)`.
For a typed error, use `StateMachineError::other(e)`. A direct `bail!` must
become a typed return, or stay inside the anyhow helper. `apply`, `snapshot_at`,
`capture_at`, the applied watermark, snapshot cuts and persisted indexes now
also use raft-core's `Index`; node identities use `NodeId`, and terms use `Term`.

**Downcasts.** An anyhow error converted by `From<anyhow::Error>` is retained
whole and returned by the host unchanged. A typed `other(E)` is reached through
`StateMachineError::Other`. `ProposalBackpressure` still downcasts directly
from `RaftHost::propose`, including through anyhow context wrappers. Nine tests
in `port_error_downcast` cover these cases and membership errors. This preserves
lumen's admission and `RaftWriteSink::submit` handling. Its coordinator errors
do not cross a Raft port.

**Private configuration fields.**

- Build `HostConfig` from `default()` with `with_tick`, `with_pump`,
  `with_rpc_timeout`, `with_propose_timeout`, `with_snapshot`. Read getters
  of those field names.
- Build `ClusterDims` with `new(shard_count, replicas_per_shard, voter_count, pod_name)` or
  the existing environment reader. Read `shard_count()`,
  `replicas_per_shard()`, `voter_count()` and `pod_name()`.
- `FenceToken`, `ActiveAssignment`, `PeerAddr`, `ClusterStateView` and
  `RaftStatus` keep their public fields because they are wire or saved shapes.

**Group and assignment identities.** Replace `GroupId(text)` with
`GroupId::new(text)`, and `.0` with `as_str()`. `AssignmentEpoch` replaces a
`u64` alias: use `new`, `get` and `checked_next`. `FenceToken.epoch`, assignment
errors and `FencedAssignment::epoch()` use it. JSON and error text are unchanged.

**Layer wiring.** `RaftStore` implements `RaftStorage`; the HTTP peer client
implements `RaftDelivery` and the internal `PeerClient` port. Public `spawn*`,
`ReplicaHostBuilder` build and `DeterministicHost::open` wire them in `src/app`.
Their public adapter choices stay available. The inbound and outbound peer
body copies encode the same bytes, pinned by golden and pairwise tests.

**Removed items.** `ensure_static_membership_unchanged` and
`RaftStore::cache_footprint`. The old `cluster` and `group` facade paths are
also deleted; import their surviving names from the crate root.

- Affects: defer, keep, loom, lumen, relay, sift and tape. Assignment epochs
  affect defer and relay. No inspected downstream uses raft-runtime's `GroupId`.
- The `adversarial_recovery` test name and path stay in place for lumen's CI.
- The implementor-gate topic still names the external build script. Downstream
  implementations must migrate before they can pass that external build.

## server-http

**`HttpServerReport` is now a type alias.**
- `pub type HttpServerReport = server_tcp::TcpServerReport`.
- The fields, derives and `Default` are the same, so literals and field reads
  still compile.
- One case breaks: a downstream crate that implements the same trait for
  both `HttpServerReport` and `TcpServerReport` now has two impls for one
  type. No such impl was found.

**Private fields.** `HttpServerOptions` (also exported as
`H2cServerOptions`) and `TlsServerOptions`:
- Start from `Default` and chain `with_*` builders:
  - `HttpServerOptions`: `with_max_concurrent_streams`,
    `with_drain_timeout`, `with_connection_budget`, `with_drain`,
    `with_socket`, `with_connection_metrics`
  - `TlsServerOptions`: `with_http`, `with_metrics`
- Every field has a getter of the same name. `connection_budget()` returns
  `Option<&_>`.
- `HttpServerOptions::set_drain_timeout(d)` replaces the in-place write
  `opts.drain_timeout = d`.

```rust
// before
let mut opts = HttpServerOptions { drain, ..Default::default() };
opts.drain_timeout = timeout;
let TlsServerOptions { http, metrics } = tls;
// after
let mut opts = HttpServerOptions::default().with_drain(drain);
opts.set_drain_timeout(timeout);
let (http, metrics) = (tls.http(), tls.metrics());
```

- Affects: pgpool. sift uses only `Default` and is not affected.

## server-lifecycle

**Private fields.**
- `BindConfig`:
  - Build with `BindConfig::new(host, port)` (an `IpAddr` and a `u16`, a
    `const fn`) or `BindConfig::from(addr)` (a `SocketAddr`).
  - Read with `host()` and `port()`.
- `ShutdownDeadline`:
  - Build with `ShutdownDeadline::new(expires_at, total, reserve)`, which
    returns `Result<Self, DeadlineError>` and checks the reserve the way
    `from_now` does. `from_now` is unchanged.
  - Read with `expires_at()`, `total()` and `reserve()`.

```rust
// before
let bind = BindConfig { host: addr.ip(), port: addr.port() };
let left = deadline.expires_at.saturating_duration_since(now);
// after
let bind = BindConfig::from(addr);
let left = deadline.expires_at().saturating_duration_since(now);
```

- Affects: pgpool (`BindConfig`) and lumen (`ShutdownDeadline` reads).
- These keep public fields because the library builds them and your code
  only reads them: `ShutdownContext`, `ShutdownReport`, `HookOutcome`,
  `PhaseTiming`, `LifecycleObservation` and `ConnectionLimitExceeded`.

## server-tcp

**Private fields.**
- `TcpServerConfig`:
  - `new` and the `with_*` builders are unchanged.
  - Getters: `bind()`, `connection_budget()` (returns `Option<&_>`),
    `drain()`, `socket()`, `drain_timeout()`, `connection_metrics()`.
  - Functional record update (`TcpServerConfig { drain, ..config }`) becomes
    `config.with_drain(drain)`.
- `TcpSocketOptions`:
  - Start from `Default` and chain `with_backlog`, `with_reuse_addr` and
    `with_nodelay`.
  - Getters: `backlog()`, `reuse_addr()`, `nodelay()`.
- `TcpConnectionResult`:
  - Build with `TcpConnectionResult::new(terminal)`, then the
    `with_streams_{completed, admitted, active_at_drain, refused, timed_out, ambiguous}`
    builders.
  - Getters: `terminal()` and `streams_*()`.

```rust
// before
let config = TcpServerConfig { drain: drain.clone(), ..config };
if config.drain.is_draining() { … }
// after
let config = config.with_drain(drain.clone());
if config.drain().is_draining() { … }
```

- Affects: pgpool (`TcpServerConfig`).
- `TcpServerReport` and `ConnectionContext` keep public fields. They are
  outputs that the library builds.

**Errors: `TcpHandlerError`.**
- `TcpHandler::Future`, and the closure blanket impl, now resolve to
  `Result<(), TcpHandlerError>` instead of `anyhow::Result<()>`.
- `TcpHandlerError` has one transparent variant. Build it with
  `TcpHandlerError::other(e)`.
- `serve` logs a handler failure exactly as before, because the log shows
  the wrapped error's own Display.
- No `serve*` signature changed. server-tcp and server-http no longer
  depend on anyhow.

```rust
// before
impl TcpHandler for SessionHandler {
    type Future = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;
    fn handle(&self, stream: TcpStream, cx: ConnectionContext) -> Self::Future {
        Box::pin(async move { run(stream, cx).await })
    }
}
// after
impl TcpHandler for SessionHandler {
    type Future = Pin<Box<dyn Future<Output = Result<(), TcpHandlerError>> + Send>>;
    fn handle(&self, stream: TcpStream, cx: ConnectionContext) -> Self::Future {
        Box::pin(async move { run(stream, cx).await.map_err(TcpHandlerError::other) })
    }
}
```

In a closure handler, `Ok(())` needs the new type:
`Ok::<(), TcpHandlerError>(())`.

- Affects: pgpool (three `type Future` lines). Its handler bodies return
  `Ok(())` and do not change.

## service-auth

**Private fields.**

| Type | Build with | Read with |
|---|---|---|
| `TokenClaims` | `TokenClaims::new(subject, roles)` | `subject()`, `roles()` |
| `TokenReviewOutcome` | `TokenReviewOutcome::authenticated(identity, audiences)`, `TokenReviewOutcome::rejected(error)`, or `TokenReviewOutcome::new(authenticated, identity, audiences, error)` | `is_authenticated()`, `identity()`, `audiences()`, `error()` |
| `CachePolicy` | `CachePolicy::default()`, then `.with_allow_ttl(d)`, `.with_deny_ttl(d)`, `.with_stale_window(d)`, `.with_max_entries(n)` | a getter named after each field |

`TokenClaims` still deserializes from the same JSON, so a
`HashMap<String, TokenClaims>` field in your config needs no change.

```rust
// before
TokenClaims { subject: "writer".to_string(), roles: HashMap::from([("*".to_string(), Role::Write)]) }
TokenReviewOutcome {
    authenticated: true,
    identity: ReviewedIdentity { username, ..Default::default() },
    audiences: audiences.to_vec(),
    error: None,
}
TokenReviewOutcome {
    authenticated: false,
    identity: ReviewedIdentity::default(),
    audiences: Vec::new(),
    error: Some("unknown token".into()),
}
CachePolicy { allow_ttl: Duration::ZERO, deny_ttl: Duration::ZERO, stale_window: Duration::ZERO, ..CachePolicy::default() }
// after
TokenClaims::new("writer", HashMap::from([("*".to_string(), Role::Write)]))
TokenReviewOutcome::authenticated(ReviewedIdentity { username, ..Default::default() }, audiences.to_vec())
TokenReviewOutcome::rejected("unknown token")
CachePolicy::default()
    .with_allow_ttl(Duration::ZERO)
    .with_deny_ttl(Duration::ZERO)
    .with_stale_window(Duration::ZERO)
```

**Errors: `RegistryError`.** `Registry::parse` and `Registry::try_merge`
return `RegistryError` instead of `anyhow::Error`. `?` and `.context(..)`
in an `anyhow` function compile unchanged. `RegistryError` is not
`UnwindSafe` or `RefUnwindSafe`, which `anyhow::Error` was.

**Traits you implement.** `ReviewBackend` and `AccessTokenIntrospection`
are now defined in the crate's domain layer. Their paths and method types
are unchanged, so an `#[async_trait] impl ReviewBackend for ..` compiles as
before. A test in core implements it that way.

**Removed.**

| Removed | Use instead |
|---|---|
| `gcp::MetadataTokenSource`, `gcp::MetadataTokenError`, `gcp::DEFAULT_METADATA_BASE_URL` | nothing in core |
| `ReloadableRoleMapVerifier::reload_file(path)` | `reload_files(&[path.into()])` |
| `reload::spawn_registry_files_watcher(v, &paths)` | `spawn_registry_files_watcher_with_interval(v, &paths, DEFAULT_REGISTRY_FILE_WATCH_INTERVAL)` |
| `TokenRequestTarget::qualified_name()` | `format!("{}/{}", target.namespace(), target.service_account())` |

The single-file `spawn_registry_file_watcher` stays.

- Affects:
  - `TokenClaims` literals: sift
  - `TokenReviewOutcome` literals: lumen and sift
  - the `CachePolicy` literal: lumen
- Unchanged: the `TokenClaims` and registry JSON, the TokenRequest body,
  and the Display text of every error; golden tests pin them.

**Modules removed.**
- `service_auth::async_verifier`, `service_auth::role_map` and
  `service_auth::scoped` were facades over root names.
- Change `service_auth::<module>::X` to `service_auth::X`.
- Affects: lumen (`service_auth::role_map::Role` becomes
  `service_auth::Role`).
- `gcp`, `k8s`, `llm` and `reload` stay.

## service-backup

**Errors.** These returned `anyhow` and now return the crate's own error
types, all at the crate root:

| Function | Error type |
|---|---|
| `BackupSink::put`, `BackupSink::prune` | `BackupSinkError` |
| `BackupDestination::from_uri` | `DestinationError` |
| `ScheduledBackupPolicy::to_runtime_policy`, `BackupPolicy::try_from(&ScheduledBackupPolicy)` | `PolicyError` |

- Calls with `?` in an `anyhow` function, `.unwrap()` and
  `.map_err(|e| e.to_string())` compile unchanged.
- An implementation of `BackupSink` whose body uses anyhow wraps its result
  with `.map_err(BackupSinkError::other)`. No downstream repo implements
  it.
- `BackupSinkError` is not `UnwindSafe` or `RefUnwindSafe`, which
  `anyhow::Error` was.

**Private fields: `AdminSnapshotTransportConfig`.** Start from
`AdminSnapshotTransportConfig::default()` and add
`.with_connect_timeout(d)`, `.with_operation_timeout(d)`,
`.with_response_idle_timeout(d)` or `.with_max_diagnostic_bytes(n)`. Read
the values with getters named after the fields.

```rust
// before
AdminSnapshotTransportConfig { operation_timeout: LIVE_BACKUP_TIMEOUT, max_diagnostic_bytes: MAX_ERROR_BODY_BYTES, ..Default::default() }
// after
AdminSnapshotTransportConfig::default()
    .with_operation_timeout(LIVE_BACKUP_TIMEOUT)
    .with_max_diagnostic_bytes(MAX_ERROR_BODY_BYTES)
```

**Removed.**

| Removed | Use instead |
|---|---|
| `fetch_admin_snapshot(base, token)` | `AdminSnapshotTransport::new()?.fetch_exact(base, token).await?` |
| `BackupDestination::default_prefix()` | the destination's prefix, or `"backup"` when a local prefix is `None` or an S3 or GCS prefix is empty |

`fetch_exact` is stricter than the removed function, in the same ways as
the behaviour change below.

**Behaviour.** `run_admin_snapshot_backup` keeps its signature, but it now
fetches the snapshot the way `AdminSnapshotTransport` does:
- Only `200 OK` succeeds. Before, any 2xx did.
- Redirects are not followed. Before, up to 10 were.
- It times out: 10 s to connect, 30 s without body data, 2 h in all.
  Before, there was no timeout.
- The error text no longer contains the URL or the response body. It is
  one of `admin snapshot Backup returned unexpected status <status>`,
  `admin snapshot Backup request failed`,
  `admin snapshot Backup response read failed` or
  `admin snapshot client build failed`. Before, it was
  `GET <url>/admin/backup returned <status>: <body>`, and a client build
  failure panicked.

The request is the same: `GET <base>/admin/backup` with the same bearer
header. The bytes written to the sink are the same.

- Affects:
  - `fetch_admin_snapshot`: lumen, relay and tape re-export it as
    `fetch_snapshot_bytes`, and lumen calls it
  - `AdminSnapshotTransportConfig`: sift
  - the behaviour change: every `run_admin_snapshot_backup` caller, that is
    defer, and lumen, relay and tape through their `run_backup`
    re-exports. A service whose `/admin/backup` answers with a redirect or
    with a 2xx other than 200 now fails.
- Unchanged: `sink_from_destination`, `run_backup_once`,
  `AdminSnapshotRequest::with_projected_bearer`, `SUPPORTED_SCHEMES` and
  `SchemeInfo`. The CRD JSON and schemas of `BackupDestination`,
  `BackupPolicy`, `ScheduledBackupPolicy` and `RetentionPolicy` are the
  same bytes, and so is the LLM topic text; golden tests pin them.

## service-collector

**Private fields.** `RuntimeConfig`:
- Build with `RuntimeConfig::try_new(batch_size, max_record_bytes, retry, follow, follow_poll_interval)`,
  which returns `Result<Self, ConfigError>`.
- Getters have the field names. All of them return by value.
- `RuntimeConfig::validate` is removed. `try_new` runs the same checks, so a
  `RuntimeConfig` that exists is valid.

**Private fields.** `SourceProgress` and `DeliveryReceipt`:
- Build with `SourceProgress::new(start_offset, final_offset, lost_bytes, lost_sources)`
  and `DeliveryReceipt::new(accepted, duplicates)`.
- Getters have the field names. `Default` is unchanged.

```rust
// before
let config = RuntimeConfig { batch_size, max_record_bytes, retry, follow, follow_poll_interval };
config.validate()?;
let receipt = DeliveryReceipt { accepted: n, duplicates: 0 };
let start = report.progress.start_offset;
// after
let config = RuntimeConfig::try_new(batch_size, max_record_bytes, retry, follow, follow_poll_interval)?;
let receipt = DeliveryReceipt::new(n, 0);
let start = report.progress.start_offset().get();
```

**Behaviour.** An invalid config now fails in `try_new`, with the same
`ConfigError` variant. Before, it failed when `run_collector*` started,
before the first source read.

- Affects: sift.
- These keep public fields: `RunReport`, `CommitStats` and `RetryPolicy`.
  `RetryPolicy::new` still validates.

**Source positions.** `SourceProgress::new` takes `SourceOffset` for the
first two arguments. Build each with `SourceOffset::new(offset)`.
`start_offset()` and `final_offset()` return this type; use `get()` when a
report or product API needs a `u64`. Lost-byte, lost-source and delivery
counts remain `u64`. The product's associated `Cursor` type is unchanged.

## service-executor

**Private fields.** `GroupCommitConfig`:
- `new` and `with_queue_capacity` are unchanged.
- Getters: `max_delay()`, `max_items()`, `max_bytes()`, `queue_capacity()`.

No downstream change is needed: sift builds it with `new` and does not read
the fields.

## service-http

**Modules removed.** Thirteen public modules were facades over names that
the crate root already exports:
- admission, body_limit, config, content_decode, error, logging, metrics
- probes, readiness, reverse_proxy, server_timing, signal,
  weighted_admission

Change `service_http::<module>::X` to `service_http::X`; it is the same
item. `service_http::transport` stays. No downstream use of the removed
paths was found.

**Private fields.** `new` is unchanged for each type below; read the fields
through getters:
- `HttpConfig`: `host()`, `port()`, `log_level()`, `log_format()`,
  `grace_secs()`, `body_limit_bytes()`, `otlp_endpoint()`. The string
  getters return `&str` or `Option<&str>`.
- `WeightedAdmissionConfig`: `max_concurrent_per_key()`,
  `max_weight_per_window()`, `window()`, `max_keys()`.
- `ContentDecodeLimits`: `max_compressed_bytes()`, `max_decoded_bytes()`.

```rust
// before
let grace = Duration::from_secs(cfg.grace_secs);
// after
let grace = Duration::from_secs(cfg.grace_secs());
```

- Affects: sift (`HttpConfig` reads).
- The error envelopes, admission events and projection metadata are wire or
  output types and keep public fields.

## service-k8s

**Private fields: the operator contract.** Your `ManagedService` impl
builds these, so each literal becomes a constructor call:

| Type | Build with | Read with |
|---|---|---|
| `ReadinessTarget` | `ReadinessTarget::new(kind, name)` | `kind()`, `name()` |
| `PruneTarget` | `PruneTarget::new(api_version, kind, name)` | `api_version()`, `kind()`, `name()` |
| `ClusterScopedChild` | `ClusterScopedChild::new(api_version, kind, name, desired)`, then `.with_expected_labels(map)` | `api_version()`, `kind()`, `name()`, `expected_labels()`, `desired()` |
| `ReconcilePlan` | `ReconcilePlan::new(children, context)` | `children()`, `context()`, `into_parts()` |
| `ReadyFacts` | `ReadyFacts::new(map)` | `get(name)` as before, or `ready()` for the map |

The `ManagedService` trait itself is unchanged, and so are its paths
(`service_k8s::service::ManagedService` and the root).

```rust
// before
ReadinessTarget { kind: "StatefulSet", name: format!("{name}-store") }
ReconcilePlan { children, context: Value::Object(context) }
ReadyFacts { ready: all_ready }
// after
ReadinessTarget::new("StatefulSet", format!("{name}-store"))
ReconcilePlan::new(children, Value::Object(context))
ReadyFacts::new(all_ready)
```

**Private fields: `RenderCtx`.**
- Build it with `RenderCtx::new(app, manager, api_version, kind, name, ns)`.
  All six arguments are `&str`, so keep this order; it is the old field
  order.
- `.with_owner(owner)` sets the owner reference. It takes a `Value` or an
  `Option<Value>`, so an `owner: owner_ref(..)` field becomes
  `.with_owner(owner_ref(..))`, and `owner: None` can be dropped.
- Read the fields with `app()`, `manager()`, `api_version()`, `kind()`,
  `name()` and `ns()`, which return `&'a str`, and `owner()`, which returns
  `Option<&Value>`. `labels`, `selector` and `meta` are unchanged.

```rust
// before
RenderCtx { app: APP, manager: MANAGER, api_version: API_VERSION, kind: KIND, name, ns, owner: owner_ref(obj) }
format!("{}-{role}", cx.name)
// after
RenderCtx::new(APP, MANAGER, API_VERSION, KIND, name, ns).with_owner(owner_ref(obj))
format!("{}-{role}", cx.name())
```

**Private fields: RBAC and CronJob plans.**
- `RbacRulePlan::new(api_groups, resources, verbs)`, plus
  `.with_resource_names(names)`.
- `RolePlan::new(name, component)`, plus `.with_rule(rule)` for each rule.
- `RoleBindingPlan::new(name, component, role_name)`, plus
  `.with_service_account(subject)` for each subject.
- `ServiceAccountSubjectPlan::new(namespace, name)`. Note that the
  namespace comes first.
- `CronJobPlan::new(name, schedule, pod)` starts with the Kubernetes
  default history limits: 3 successful Jobs and 1 failed Job. If your
  literal set other values, add `.with_successful_jobs_history_limit(n)` or
  `.with_failed_jobs_history_limit(n)`, or the rendered CronJob changes.
- Each field has a getter of the same name, for example `rules()`,
  `verbs()`, `subjects()` and `schedule()`.

```rust
// before
render::CronJobPlan {
    name: backup_name,
    schedule: backup.schedule.clone(),
    successful_jobs_history_limit: 3,
    failed_jobs_history_limit: 3,
    pod,
}
// after
render::CronJobPlan::new(backup_name, backup.schedule.clone(), pod)
    .with_failed_jobs_history_limit(3)
```

**Certificates.** Key generation and leaf parsing moved behind two new
ports, `KeyAndCsrGenerator` and `LeafParser`. `RcgenCsrGenerator` and
`X509LeafParser` implement them. Two functions take the port as a new last
argument:
- `IssuanceRequest::build(&scope, &profile)` becomes
  `IssuanceRequest::build(&scope, &profile, &RcgenCsrGenerator)`.
- `certificate::projection::read_state(&data, &annotations)` becomes
  `read_state(&data, &annotations, &X509LeafParser)`.

`certificate::Reconciler::new` keeps its signature, so a service that only
runs the reconciler changes nothing. `Issuer` and `SecretStore` now spell
their return type as `Pin<Box<dyn Future<Output = T> + Send + 'a>>`, which is
the same type as `BoxFuture<'a, T>`, so implementations still compile.

**Removed.** These had no callers.

| Removed | Use instead |
|---|---|
| `WorkloadIdentityTokenSource::with_sts_endpoint` | nothing |
| `NetworkPeerPlan::same_namespace()` | `NetworkPeerPlan::SameNamespace` |
| `NetworkPeerPlan::with_except` | nothing |
| `ShardSplitPlan::requires_split()` | `plan.desired_shard_count > plan.current_shard_count` |
| `ReplicaLayerPlan::requires_membership_change()` | `plan.current_replicas_per_shard != plan.desired_replicas_per_shard` |
| `certificate::reconcile::PROJECTED_KEYS` | nothing |
| `EphemeralIssuer::anchor_pem` | nothing |
| `certificate::KubernetesStoreError` | `StoreError` |
| `ConditionStatus::from_bool(b)` | `if b { ConditionStatus::True } else { ConditionStatus::False }` |

**Behaviour.** A reconcile reads the clock for the condition timestamps
just before it writes the `Reconciled` Event, instead of just after. The
request order is unchanged.

- Affects:
  - `RenderCtx`: 9 literals and about 73 field reads in defer, keep, loom,
    lumen, pgpool, relay, sift and tape
  - the operator contract types: the same repos, most of them in
    `reconcile.rs` and their operator tests
  - the RBAC and CronJob plans: sift only
- Unchanged:
  - `service_k8s::run`, `controller::{run, reconcile_once, Error}`,
    `lease::{spawn, Election}` (so `pub use service_k8s::lease::*` exports
    the same names), and `Election`'s public fields
  - the wire types `Condition`, `ClusterSpec` and `ResourceSpec`, the
    capacity plans, `ContainerPlan`, `StatefulInstancePlan` and the other
    render plans, which keep public fields
  - the CRD schemas, conditions, Secrets and status patch bytes, which
    golden tests pin

## service-observability

**Private fields.** `ObservabilityConfig`:
- `new` is unchanged.
- Getters: `log_level()` (`&str`), `log_format()` and `otlp_endpoint()`
  (`Option<&str>`).

No downstream use was found. `FilesystemUsage`, `ProcessUsage`, the
lifecycle snapshot and the `ServiceLog*V1` wire types keep public fields.

## service-projection

**Errors: `ProjectionError`.** These port methods returned `anyhow::Result`
and now return `Result<_, ProjectionError>`:
- `Projection`: `apply_idempotent`, `snapshot`, `restore`,
  `checkpoint_committed` and `semantic_digest`
- `ProjectionSource`: `read_after` and `open_read_session`
- `ProjectionReadSession`: `read_next`

`ProjectionError` has two variants:
- `InvalidName`, for a descriptor with a bad name.
- `Other`, a transparent box for your own errors. Build it with
  `ProjectionError::other(e)`. `e` can be an `anyhow::Error`; the message and
  the source chain are kept.

The registry and the handle still return `anyhow`, and so does the
`register` factory.

```rust
// before
fn read_after(&self, cursor: u64, limit: usize) -> anyhow::Result<Vec<Record>> {
    self.journal.read_after(cursor, limit)
}
// after
fn read_after(&self, cursor: ProjectionCursor, limit: usize) -> Result<Vec<Record>, ProjectionError> {
    self.journal.read_after(cursor.get(), limit).map_err(ProjectionError::other)
}
```

**Private fields.** `ProjectionDescriptor`:
- Build with `ProjectionDescriptor::try_new(name, schema_version, retention)`,
  which returns `Result<Self, ProjectionError>` and rejects a blank name or a
  name containing `/` or NUL.
- Read with `name()`, `schema_version()` and `retention()`.
- The JSON and the OpenAPI schema are unchanged.

In `fn descriptor(&self) -> ProjectionDescriptor`, where the name is a
constant, call
`ProjectionDescriptor::try_new(ProjectionName::new(NAME), VERSION, retention).expect("projection name is a valid constant")`.

**Signature.** `ProjectionCheckpoint::empty(&descriptor)` takes the time as
a second argument: `ProjectionCheckpoint::empty(&descriptor, Utc::now())`.

- Affects: sift (its projection runtime implements the three ports and
  builds descriptors).
- `ProjectionRegistry::new` is unchanged.
- The checkpoint and state file bytes are unchanged. Golden tests pin them.

**Identity and position types.**

| Type | Build | Read |
|------|-------|------|
| `ProjectionName` | `ProjectionName::new(name)` | `as_str()` |
| `ProjectionEventId` | `ProjectionEventId::new(id)` | `as_str()` |
| `ProjectionCursor` | `ProjectionCursor::new(cursor)` | `get()` |
| `SourceGeneration` | `SourceGeneration::new(generation)` | `get()` |

`ProjectionDescriptor::try_new` takes `ProjectionName`; `name()` returns
`&ProjectionName`. `ProjectionRecord::projection_cursor` returns
`ProjectionCursor`; `projection_event_id` returns an owned `ProjectionEventId`.
`ProjectionSource::current_cursor`, `read_after` and `open_read_session` use
`ProjectionCursor`; `generation` returns `SourceGeneration` and defaults to 0.
The handle and registry cursor results, and `wait_for_min_cursor`'s input,
are `ProjectionCursor`. Checkpoints, lag reports and rebuild reports use
these types too. Use `cursor.distance_since(earlier)` for an event count.
The three OpenAPI schemas, checkpoint JSON and durable state files are pinned
to their exact pre-newtype bytes.

## storage-durable

**Errors: `DataRootError`.** `DataRootPolicy` and `DataRoot` returned
`anyhow` and now use `DataRootError`:
- the port methods `create_manifest` and `validate_manifest`
- `legacy_error`, which now returns a `DataRootError`
- `DataRoot::open` and `DataRoot::replace_manifest`

The variants are what storage-durable reports itself:
- `Symlink`, `NotADirectory`, `NotARegularFile` and `UnsafeDirectory`
- `LegacyData`
- `ManifestDecode` and `ManifestEncode`
- `Io { context, source }`
- a transparent `Other`, built with `DataRootError::other(e)`, for your
  policy's own errors

The Display and `{:#}` text is unchanged.

```rust
// before
fn validate_manifest(&self, m: &Self::Manifest) -> anyhow::Result<()> {
    if m.role != self.role { bail!("data root belongs to role {}", m.role) }
    Ok(())
}
fn legacy_error(&self, marker: &Path) -> anyhow::Error {
    anyhow!("legacy data at {}", marker.display())
}
// after
fn validate_manifest(&self, m: &Self::Manifest) -> Result<(), DataRootError> {
    if m.role != self.role {
        return Err(DataRootError::other(format!("data root belongs to role {}", m.role)));
    }
    Ok(())
}
fn legacy_error(&self, marker: &Path) -> DataRootError {
    DataRootError::other(format!("legacy data at {}", marker.display()))
}
```

- A body that uses anyhow's `.with_context(..)?` can stay in a private
  helper that returns `anyhow::Result`. The trait method then calls
  `self.helper(root).map_err(DataRootError::other)`.
- `DataRoot::open(..)?` in an anyhow function compiles unchanged. A tail
  expression `self.inner.replace_manifest(m)` becomes
  `Ok(self.inner.replace_manifest(m)?)`.

**Behaviour.** For an I/O failure, `downcast_ref::<std::io::Error>()`
called directly on the `anyhow::Error` from `DataRoot::open` no longer finds
the error. The top-level error is now a `DataRootError`, with the
`io::Error` as its source. Two ways still work, and a test covers both:
- walking `err.chain()`
- `err.downcast_ref::<DataRootError>()`

No downstream code downcasts these errors.

**Private fields.** `LogFrame`:
- Build with `LogFrame::new(seq, payload)`.
- Read with `seq()` and `payload()` (`&[u8]`), or move the payload out with
  `into_payload()`.
- The derives and the frame bytes are unchanged. `MappedLogFrame` is
  unchanged.

```rust
// before
let frame = LogFrame { seq: 1, payload };
apply(frame.seq, decode(&frame.payload)?);
let text = String::from_utf8(frame.payload)?;
// after
let frame = LogFrame::new(1, payload);
apply(frame.seq(), decode(frame.payload())?);
let text = String::from_utf8(frame.into_payload())?;
```

**Removed.** Four methods had no callers. Use the sibling each one
duplicated:

| Removed | Use instead |
|---|---|
| `FramedLogCursor::reread_frame_at(off)` | `reread_large_mapped_frame_at(off)` |
| `FramedLogCursor::reread_mapped_frame_at(off)` | `reread_large_mapped_frame_at(off)` |
| `FramedLogWriter::finish_sync(plan)` | `plan.sync_off_lock()?` then `writer.complete_sync(plan)` |
| `GenerationStore::commit_from_current(staged)` | `commit_from_current_with_publication_guard(staged, \|\| Ok(()))` |

- Affects:
  - sift: `DataRootPolicy` and `LogFrame` reads
  - lumen and tape: `LogFrame` literals and reads
- Unchanged:
  - the path helpers `reject_symlink`, `set_private_directory_mode` and
    `set_private_file_mode`, which still return `anyhow::Result`
  - the framed-log bytes and `layout.json`, which golden tests pin

## storage-object

**Private fields.**
- `ObjectMeta`:
  - Build with `ObjectMeta::new(key, size, content_type, version)`, then
    `with_etag(Option<String>)` and `with_updated(Option<String>)`.
  - Read with `key()`, `size()`, `content_type()`, `version()`, `etag()` and
    `updated()`.
- `Object`:
  - Build with `Object::new(meta, bytes)`.
  - Borrow with `meta()` and `bytes()`, or move out with `into_parts()`.

```rust
// before
let meta = ObjectMeta { key, size, content_type, version, etag: Some(tag), updated: None };
let object = Object { meta, bytes };
let body = object.bytes;
// after
let meta = ObjectMeta::new(key, size, content_type, version).with_etag(Some(tag));
let object = Object::new(meta, bytes);
let (_, body) = object.into_parts();
```

- Affects: sift (its test `ObjectStore` implementation).
- The `ObjectMeta` JSON is unchanged.

## storage-segment

**Removed.** The `SegmentStore` trait. It had no implementations.

**Private fields.** `CatalogEntry`:
- Build with `CatalogEntry::try_new(key, value)`, which returns
  `storage_segment::Result<Self>`.
- It rejects an empty key, a key over 1024 bytes or a key containing NUL,
  with `SegmentError::InvalidCatalogKey`. Before, the same key failed with
  the same error later, in `build`, `upsert` or the streaming build.
- Borrow with `key()` and `value()`, or move out with `into_parts()`.

```rust
// before
let entry = CatalogEntry { key, value };
let text = String::from_utf8(entry.value)?;
// after
let entry = CatalogEntry::try_new(key, value)?;
let (_, value) = entry.into_parts();
let text = String::from_utf8(value)?;
```

**Changed types.**
- `ArchivedObject.version` is a new `storage_segment::ArchivedObjectVersion`,
  not `storage_object::ObjectVersion`.
  - It serializes as the same bare string.
  - Build it with `ArchivedObjectVersion::new(s)` and read it with
    `as_str()`.
- `SegmentError::ObjectStore` holds a
  `Box<dyn std::error::Error + Send + Sync>`, not an `ObjectStoreError`.
  - The Display text is the same.
  - To reach the inner error, use
    `err.downcast_ref::<storage_object::ObjectStoreError>()`.
  - A side effect is that `SegmentError` and `StreamingCatalogAbort` are no
    longer `UnwindSafe` or `RefUnwindSafe`.

- Affects: sift (`CatalogEntry` literals and field reads).
- `PagedCatalog::new`, `PagedCatalog::with_page_bytes` and
  `ArchiveCoordinator::new` are unchanged.
- The catalog pages, page keys and archive receipts are unchanged, byte for
  byte. Golden tests pin them.

## surface

**Private fields.** `Props`:
- Start from `Props::default()` and chain one `with_<field>` builder per
  field. A builder takes the plain value, not an `Option`: the string fields
  take `impl Into<String>`, the callbacks take `Callback<_>`, and `checked`
  and `disabled` take `bool`.
- Every field has a getter of the same name:
  - string fields return `Option<&str>`
  - callbacks return `Option<&Callback<_>>`
  - `checked()` returns `Option<bool>`, and `disabled()` returns `bool`
- There are no setters. To forward an `Option`, add the builder only when
  the value is present.

**Private fields.** `Component`:
- Build with `Component::new(name, render, props)`, a `const fn`.
- Read the name with `name()`. `render()` calls the render function with the
  props.
- There are no `props()` or `render_fn()` getters.

```rust
// before
let props = Props {
    class_name: Some("jet-action".to_string()),
    on_click: Some(cb),
    ..Default::default()
};
if let Some(id) = &props.id { … }
let c = Component { name: "Counter", render: render_counter, props: Rc::new(p) };
let tree = (c.render)(&c.props);

// after
let props = Props::default().with_class_name("jet-action").with_on_click(cb);
if let Some(id) = props.id() { … } // id is &str now, not &String
let c = Component::new("Counter", render_counter, Rc::new(p));
let tree = c.render();
```

Read patterns:

| Before | After |
|---|---|
| `props.x.as_deref()` | `props.x()` |
| `props.x.clone()`, where `x: Option<String>` | `props.x().map(str::to_string)` |
| `props.on_click.clone()` | `props.on_click().cloned()` |
| `props.checked`, `props.disabled` | `props.checked()`, `props.disabled()` |

- Affects: jet only.
  - jet-wasm builds and reads these types in its renderer, React apps,
    debug module and tests.
  - jet's TSX-to-Rust emitter (`src/tsx_to_rust/emit.rs`) generates `Props`
    and `Component` literals, and the tests that compare its output hold
    them as expected strings. The emitter must print builder chains and
    `Component::new(…)` instead.
- The `SurfaceSnapshot` JSON, and the `SurfaceNode`, `SurfaceProps` and
  `SurfaceRect` types, are unchanged. A golden test pins the JSON.

## transport-h2c

**Private fields.**
- `ManagerConfig`:
  - Each of its 13 fields has a `with_<field>` builder and a `<field>()`
    getter.
  - `Default` and `for_concurrency` are unchanged.
- `ConnectionOptions`:
  - Build with `ConnectionOptions::new(max_concurrent_streams)`, a
    `const fn`.
  - Read with `max_concurrent_streams()`.

```rust
// before
ManagerConfig { max_connections: 8, ..Default::default() }
// after
ManagerConfig::default().with_max_connections(8)
```

No downstream use was found.

## transport-otlp

**Removed.** `DecodedPayload::signal()`. Match on the `DecodedPayload`
variants instead (`Logs`, `Metrics`, `Traces`, or the `signal` field of
`Json`). No downstream use was found.

## ui-runtime

**Newtype.**
- `FiberId(pub u64)` is now `FiberId(u64)`. Build it with `FiberId::new(n)`
  and read it with `id.get()`; both are `const fn`.
- The derives are the same, so its Debug output is still `FiberId(n)`.

**`debug` feature.**
- `DebugFiberMeta.id` is a `FiberId`, not a `u64`.
- `debug_snapshot_hooks` takes a `FiberId`.

```rust
// before
let hooks = debug_snapshot_hooks(fiber_id as u64);
DebugFiber { id: meta.id, … }
// after
let hooks = debug_snapshot_hooks(FiberId::new(fiber_id as u64));
DebugFiber { id: meta.id.get(), … }
```

- Affects: jet-wasm's debug module. It re-exports `ui_runtime::*`, so
  `FiberId` is in scope through the same glob.
- Root names are unchanged. The code moved into private modules, so
  `pub use ui_runtime::*` sees the same set of names.
