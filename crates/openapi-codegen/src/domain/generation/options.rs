use std::path::{Path, PathBuf};

use crate::domain::TargetProfile;

/// Target language for the generated client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// TypeScript: types + a typed fetch/axios client + TanStack Query hooks.
    #[default]
    Ts,
    /// Python: pydantic models + a generated sync/async HTTP/2 runtime.
    Py,
    /// Rust: serde models + a reqwest client.
    Rust,
}

impl Lang {
    /// Stable language identifier used in generation manifests.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ts => "typescript",
            Self::Py => "python",
            Self::Rust => "rust",
        }
    }
}

/// HTTP runtime backend for the generated TypeScript client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpClient {
    /// Native `fetch` (zero runtime dependency).
    #[default]
    Fetch,
    /// `axios` (peer dependency of the generated output).
    Axios,
}

/// What the generator emits, selected by CLI flags.
///
/// Build it with [`GenOptions::new`] and change the optional parts with the
/// `with_*` builders:
///
/// ```
/// use openapi_codegen::{GenOptions, HttpClient, Lang};
///
/// let opts = GenOptions::new(Lang::Ts, "openapi.json", "clients/ts", "createClient")
///     .with_http_client(HttpClient::Axios);
/// assert!(opts.emit_hooks());
/// assert_eq!(opts.target(), None);
/// ```
#[derive(Debug, Clone)]
pub struct GenOptions {
    lang: Lang,
    target: Option<TargetProfile>,
    spec_path: PathBuf,
    out_dir: PathBuf,
    client_name: String,
    http_client: HttpClient,
    emit_types: bool,
    emit_client: bool,
    emit_hooks: bool,
}

impl GenOptions {
    /// Options for one generation run: no versioned target, the `fetch`
    /// HTTP runtime, types and client emitted, and TanStack Query hooks
    /// emitted only for [`Lang::Ts`] (a TypeScript-only concern).
    pub fn new(
        lang: Lang,
        spec_path: impl Into<PathBuf>,
        out_dir: impl Into<PathBuf>,
        client_name: impl Into<String>,
    ) -> Self {
        Self {
            lang,
            target: None,
            spec_path: spec_path.into(),
            out_dir: out_dir.into(),
            client_name: client_name.into(),
            http_client: HttpClient::Fetch,
            emit_types: true,
            emit_client: true,
            emit_hooks: lang == Lang::Ts,
        }
    }

    /// Set the versioned language/toolchain contract. Without one, the legacy
    /// generated files are kept and no target manifest is emitted. The
    /// profile is checked against [`lang`](Self::lang) when generation runs.
    pub fn with_target(mut self, target: TargetProfile) -> Self {
        self.target = Some(target);
        self
    }

    /// Set the HTTP runtime backend of the generated TypeScript client.
    pub fn with_http_client(mut self, http_client: HttpClient) -> Self {
        self.http_client = http_client;
        self
    }

    /// Set whether the model types are emitted.
    pub fn with_emit_types(mut self, emit_types: bool) -> Self {
        self.emit_types = emit_types;
        self
    }

    /// Set whether the client is emitted.
    pub fn with_emit_client(mut self, emit_client: bool) -> Self {
        self.emit_client = emit_client;
        self
    }

    /// Set whether TanStack Query hooks are emitted (TypeScript only).
    pub fn with_emit_hooks(mut self, emit_hooks: bool) -> Self {
        self.emit_hooks = emit_hooks;
        self
    }

    /// Target language for the generated client.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Versioned language/toolchain contract, if any.
    pub fn target(&self) -> Option<TargetProfile> {
        self.target
    }

    /// Path of the OpenAPI document that `run` reads.
    pub fn spec_path(&self) -> &Path {
        &self.spec_path
    }

    /// Directory that `run` writes the generated files into.
    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    /// Name of the generated client factory.
    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    /// HTTP runtime backend of the generated TypeScript client.
    pub fn http_client(&self) -> HttpClient {
        self.http_client
    }

    /// Whether the model types are emitted.
    pub fn emit_types(&self) -> bool {
        self.emit_types
    }

    /// Whether the client is emitted.
    pub fn emit_client(&self) -> bool {
        self.emit_client
    }

    /// Whether TanStack Query hooks are emitted.
    pub fn emit_hooks(&self) -> bool {
        self.emit_hooks
    }
}
