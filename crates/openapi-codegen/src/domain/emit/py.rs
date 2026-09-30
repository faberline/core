//! Python emitter: read an OpenAPI 3.0/3.1/3.2 document and emit pydantic v2
//! models plus a typed sync/async HTTP/2 client runtime.
//!
//! Pipeline: parse → `models.py` (BaseModel per component schema) +
//! `h2c_runtime.py` (generated h2c + TLS ALPN h2 runtime) + `client.py`
//! (one sync and async `Client` method per operation) + `__init__.py`.
//!
//! OpenAPI 3.2 `query` operations (HTTP QUERY, RFC 10008) emit a client
//! method that calls `self._client.request("QUERY", ..., json=...)` — the
//! generated `h2c_runtime.py`/any httpx-like injected client accepts
//! arbitrary method strings. `Client(..., use_post_fallback=True)` (also on
//! `AsyncClient`) flips that call to `POST` against the operation's
//! documented twin path at request time.

pub mod client_emit;
pub mod models_emit;
pub mod pymap;
pub mod runtime_emit;

use crate::domain::ir::build_type_map;
use crate::domain::ir::openapi::Spec;
use crate::domain::ir::operations;
use crate::domain::{FileBearerAuth, GenOptions, GeneratedFile, GeneratedOutput, PythonTarget};

use super::SpecParseError;

/// Pure Python generation: spec JSON text → in-memory files. No filesystem access.
pub fn generate(spec_json: &str, opts: &GenOptions) -> Result<GeneratedOutput, SpecParseError> {
    generate_impl(spec_json, opts, None, None)
}

pub fn generate_with_file_bearer_auth(
    spec_json: &str,
    opts: &GenOptions,
    auth: &FileBearerAuth,
) -> Result<GeneratedOutput, SpecParseError> {
    generate_impl(spec_json, opts, None, Some(auth))
}

/// Profile-aware Python generation used by the public target-profile API.
pub fn generate_for_target(
    spec_json: &str,
    opts: &GenOptions,
    target: PythonTarget,
) -> Result<GeneratedOutput, SpecParseError> {
    generate_impl(spec_json, opts, Some(target), None)
}

pub fn generate_for_target_with_file_bearer_auth(
    spec_json: &str,
    opts: &GenOptions,
    target: PythonTarget,
    auth: &FileBearerAuth,
) -> Result<GeneratedOutput, SpecParseError> {
    generate_impl(spec_json, opts, Some(target), Some(auth))
}

fn generate_impl(
    spec_json: &str,
    opts: &GenOptions,
    target: Option<PythonTarget>,
    auth: Option<&FileBearerAuth>,
) -> Result<GeneratedOutput, SpecParseError> {
    let spec: Spec = serde_json::from_str(spec_json).map_err(SpecParseError::new)?;
    let tm = build_type_map(&spec);
    let ops = operations::build(&spec);

    let mut files = Vec::new();
    if opts.emit_types {
        files.push(GeneratedFile {
            rel_path: "models.py".to_string(),
            contents: models_emit::emit(&spec, &tm, target),
        });
    }
    if opts.emit_client {
        files.push(GeneratedFile {
            rel_path: "h2c_runtime.py".to_string(),
            contents: runtime_emit::emit(target),
        });
        files.push(GeneratedFile {
            rel_path: "client.py".to_string(),
            contents: client_emit::emit(&ops, &tm, target, auth),
        });
    }
    files.push(GeneratedFile {
        rel_path: "__init__.py".to_string(),
        contents: emit_init(opts),
    });
    Ok(match target {
        Some(target) => GeneratedOutput::for_target(files, crate::TargetProfile::Python(target)),
        None => GeneratedOutput::legacy(files),
    })
}

fn emit_init(opts: &GenOptions) -> String {
    let mut out = String::from(models_emit::HEADER);
    if opts.emit_types {
        out.push_str("from .models import *  # noqa: F401,F403\n");
    }
    if opts.emit_client {
        out.push_str("from .client import AsyncClient, Client  # noqa: F401\n");
        out.push_str("from .h2c_runtime import AsyncH2CClient, AsyncH2CConnection, AsyncH2CStream, H2CClient, H2CConnection, H2CResponse, H2CStream  # noqa: F401\n");
    }
    out
}

#[cfg(test)]
mod tests;
