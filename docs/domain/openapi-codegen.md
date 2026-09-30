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
  parts to emit (types, client, hooks). Built with
  `GenOptions::new(lang, spec_path, out_dir, client_name)` and the `with_*`
  builders (target, HTTP client, emit flags); read through getters named
  after each field.
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
CLI entry touch the file system. `run` is wiring: it lives in the `src/app`
composition root, reads the spec, calls generation and writes the output.

## Invariants

- `generate` and its variants map spec JSON text to an in-memory
  `GeneratedOutput` without file-system access.
- Without a target profile the output is the legacy output and has no manifest.
  With one, a profile for another language than `GenOptions::lang()`, or one
  that conflicts with `GenOptions::target()`, is rejected before any parsing.
- Only the `*_with_file_bearer_auth` entry points add credential reading; plain
  `generate` output stays byte-for-byte unchanged.
- `FileBearerAuth::new` requires a non-empty UTF-8 token path, a hostname suffix
  of one leading dot and lowercase DNS labels, and at least one scheme, and
  returns a `FileBearerAuthError` naming the first rule broken.
- Domain errors are typed: the `emit::{ts, py, rust}` `generate*` functions
  return `SpecParseError`, `TargetProfile::from_id` and `FromStr` return
  `UnknownTargetProfile`, and `TargetPolicy::resolve` returns
  `TargetPolicyError`. The root `generate*` functions, `run` and
  `TargetPolicy::from_toml` keep `anyhow` and pass the text on unchanged.
- `write_to_dir` refuses an absolute generated path or one with a `..`
  component before it writes any file.
- Only `query` operations get a POST twin: the `x-post-twin` extension, else the
  operation's own path. The `openapi` version string is never gated.

## Published language

No other core context depends on openapi-codegen. Downstream CLIs import from
the crate root: `generate` or `generate_for_target` with `GenOptions` (built
with `GenOptions::new`), `Lang`,
`HttpClient`, `TargetPolicy` and `MANIFEST_FILE`; lumen also uses
`generate_for_target_with_file_bearer_auth`, `FileBearerAuth` and
`llm::topic`. Three public modules keep their paths because they hold names
the root does not re-export (`src/api/`): `ir`, `emit` and `llm`; `ir` and
`emit` have no known external users. P2 deleted the old module `target`: every
name in it is at the crate root. `llm` is the only use of cli-std: an llm
topic (v1).

## Exceptions and debts

- **Checker exceptions:** none. P2 moved the CLI entry `run`, which names
  `GenOptions` and `MANIFEST_FILE`, from `interfaces` into the `src/app`
  composition root, which the checker does not check.
- **Tracked:** `anyhow` in the public signatures of `TargetPolicy::from_toml`
  (infrastructure) and the root `generate*` functions (application) (ADR D4).
