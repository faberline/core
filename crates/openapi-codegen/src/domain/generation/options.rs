use std::path::PathBuf;

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
#[derive(Debug, Clone)]
pub struct GenOptions {
    /// Target language for the generated client.
    pub lang: Lang,
    /// Versioned language/toolchain contract. `None` preserves the legacy
    /// generated files and does not emit a target manifest.
    pub target: Option<TargetProfile>,
    pub spec_path: PathBuf,
    pub out_dir: PathBuf,
    pub client_name: String,
    pub http_client: HttpClient,
    pub emit_types: bool,
    pub emit_client: bool,
    pub emit_hooks: bool,
}
