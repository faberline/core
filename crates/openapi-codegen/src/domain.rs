//! The generator core: generation options and results, versioned target
//! profiles, the language-neutral OpenAPI IR, and the per-language emitters
//! that render it as source text.

pub(crate) mod emit;
mod generation;
pub(crate) mod ir;
mod target;

pub use generation::{
    FileBearerAuth, FileBearerScheme, GenOptions, GeneratedFile, GeneratedOutput,
    GenerationManifest, HttpClient, Lang, MANIFEST_FILE,
};
pub use target::{
    PythonTarget, RustTarget, TargetPolicy, TargetProfile, TargetRequirements, TypeScriptTarget,
};
