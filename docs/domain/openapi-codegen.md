# openapi-codegen

openapi-codegen turns one OpenAPI 3.0, 3.1 or 3.2 document into a typed
TypeScript, Python or Rust client. It models the document, a language-neutral
operation IR, versioned language targets, and the generated files with their
target manifest. It does not decide who may call an API or how a caller gets a
credential. No core crate depends on it; the service CLIs of downstream repos
defer, keep, loom, lumen, relay, sift and tape use it to generate their clients.

**Form:** layered · **Depends on:** cli-std · **Crate:** [`crates/openapi-codegen`](../../crates/openapi-codegen)

## Model

- **GenOptions** — one generation request: `Lang` (`Ts`, `Py`, `Rust`, which
  selects the emitter), an optional target profile, spec and output paths,
  client name, `HttpClient` (`Fetch` or `Axios`, TypeScript only) and which
  parts to emit (types, client, hooks).
- **Target profile** — `TargetProfile`: one of `PythonTarget` (3.11–3.14),
  `TypeScriptTarget` (5.0) or `RustTarget` (2021, 2024), with a stable id such
  as `python-3.12`. It switches on version-dependent syntax (PEP 695 aliases
  from Python 3.12, escaping `gen` in Rust 2024) and implies
  `TargetRequirements`: compiler, minimum version, runtime dependencies.
- **Target policy** — `TargetPolicy`: a project's pinned profile per language,
  read from the `[targets]` table of a `codegen.toml`, with an optional override.
- **OpenAPI document model** — `ir::openapi::Spec` (`PathItem`, `Operation`,
  `Schema`, `RefOr`): a tolerant serde subset of OpenAPI, including the 3.2
  `query` keyword and `additionalOperations`.
- **Operation IR** — `OperationIR` (with `ParamIR`, `BodyIR`): the structural
  shape of one operation, holding schema references and never language types.
  `TypeMap` gives every component schema one collision-free PascalCase name
  shared by all emitters under `emit::{ts, py, rust}`.
- **POST twin** — the path a generated `QUERY` method calls with `POST` when
  the client's runtime fallback is on.
- **File-bearer auth** — `FileBearerAuth` (with `FileBearerScheme`): a
  validated, generation-time opt-in that makes the client reread a bearer-token
  file before each eligible request, for one DNS suffix and chosen schemes.
- **Generated output** — `GeneratedOutput`: the `GeneratedFile`s plus the
  selected target and its requirements. `GenerationManifest` is the
  `.openapi-codegen.json` (`MANIFEST_FILE`) record of that target contract.

## Ports

None. Generation is pure; only `GeneratedOutput::write_to_dir` and the `run`
CLI entry touch the file system.

## Invariants

- `generate` and its variants map spec JSON text to an in-memory
  `GeneratedOutput` without file-system access.
- Without a target profile the output is the legacy output and has no manifest.
  With one, a profile for another language than `GenOptions::lang`, or one that
  conflicts with `GenOptions::target`, is rejected before any parsing.
- Only the `*_with_file_bearer_auth` entry points add credential reading; plain
  `generate` output stays byte-for-byte unchanged.
- `FileBearerAuth::new` requires a non-empty UTF-8 token path, a hostname suffix
  of one leading dot and lowercase DNS labels, and at least one scheme.
- `write_to_dir` refuses an absolute generated path or one with a `..`
  component before it writes any file.
- Only `query` operations get a POST twin: the `x-post-twin` extension, else the
  operation's own path. The `openapi` version string is never gated.

## Published language

No other core context depends on openapi-codegen. Downstream CLIs import from
the crate root: `generate` or `generate_for_target` with `GenOptions`, `Lang`,
`HttpClient`, `TargetPolicy` and `MANIFEST_FILE`; lumen also uses
`generate_for_target_with_file_bearer_auth`, `FileBearerAuth` and
`llm::topic`. P1 keeps every root re-export, and the old modules `ir`, `emit`,
`llm` and `target` stay as compatibility facades; `ir` and `emit` have no known
external users. `llm` is the only use of cli-std: an llm topic (v1).

## Exceptions and debts

- **Checker exceptions (P1):**
  - B2 (`anyhow`): the per-language `generate*` functions,
    `FileBearerAuth::new` and the target-profile parsers
    (`TargetProfile::from_id`, `FromStr`) are in the domain and return
    `anyhow::Result`. P2 gives the domain a `thiserror` error enum (ADR D4).
  - B3 `interfaces->domain`: the CLI entry `run` builds `GenOptions` and
    prints the manifest path, both domain items. P2 lets the application layer
    take a CLI request and return the written paths.
- **Tracked for P2:** `GenOptions` public fields, built with struct literals by
  defer, keep, loom, lumen, relay, sift and tape. `anyhow` in public
  signatures: `TargetProfile: FromStr<Err = anyhow::Error>`,
  `FileBearerAuth::new`, `TargetPolicy::from_toml` and every `generate*`
  function (ADR D4).
