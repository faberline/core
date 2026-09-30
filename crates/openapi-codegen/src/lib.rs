//! `openapi-codegen` — generate a typed API client from an OpenAPI 3.0, 3.1,
//! or 3.2 document, in TypeScript, Python, or Rust. Reusable polyglot codegen
//! core, extracted from `jet codegen openapi` so any CLI can compose it.
//!
//! Architecture: a language-neutral [`ir`] (document model, naming, type-name
//! map) feeds a per-language emitter under [`emit`]. The target language is
//! [`GenOptions::lang`]:
//! - [`Lang::Ts`]  → TypeScript: types + fetch/axios client + TanStack Query hooks
//! - [`Lang::Py`]  → Python: pydantic models + generated sync/async HTTP/2 runtime
//! - [`Lang::Rust`]→ Rust: serde models + reqwest client
//!
//! A versioned target ([`TargetProfile`], chosen through [`TargetPolicy`]) is
//! intentionally separate from [`Lang`]: `Lang` selects an emitter, while a
//! target selects the minimum language/toolchain contract and any syntax that is
//! safe to use for that contract.
//!
//! OpenAPI 3.2 support: the `query` path-item keyword (RFC 10008's HTTP
//! `QUERY` method) is a first-class [`ir::operations::OperationIR`] method
//! alongside `get`/`post`/etc., and `additionalOperations` entries pass
//! through without choking (parse-don't-crash for methods beyond `QUERY`).
//! Every generated `QUERY` client method also carries a POST-twin fallback
//! (epic #1296 policy): a per-client runtime option routes the call through
//! `POST` against a documented twin path instead of HTTP `QUERY` — see each
//! emitter's `client_emit` module and the crate README's "OpenAPI 3.2 and
//! HTTP QUERY" section for the exact mechanism per language. There is no
//! hard version gate: `Spec.openapi` is parsed as an opaque string, so 3.0,
//! 3.1, and 3.2 documents (and any `3.x.y` string) all parse today.
//!
//! [`generate`] is the pure core (spec text → in-memory files, no I/O); [`run`]
//! is the filesystem-writing CLI entry.

mod api;
mod application;
mod domain;
mod infrastructure;
mod interfaces;

pub use api::{emit, ir, llm};
pub use application::{
    generate, generate_for_target, generate_for_target_with_file_bearer_auth,
    generate_with_file_bearer_auth,
};
pub use domain::ir::{build_type_map, TypeMap};
pub use domain::{
    FileBearerAuth, FileBearerScheme, GenOptions, GeneratedFile, GeneratedOutput,
    GenerationManifest, HttpClient, Lang, MANIFEST_FILE,
};
pub use domain::{
    PythonTarget, RustTarget, TargetPolicy, TargetProfile, TargetRequirements, TypeScriptTarget,
};
pub use interfaces::run;

#[cfg(test)]
mod tests;
