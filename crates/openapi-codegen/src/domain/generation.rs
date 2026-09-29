//! What a generation run is asked for and what it produces: the target
//! language, the TypeScript HTTP backend, the file-backed bearer-token
//! opt-in, the in-memory generated files, and the contract manifest.

mod file_bearer_auth;
mod options;
mod output;

pub use file_bearer_auth::{FileBearerAuth, FileBearerScheme};
pub use options::{GenOptions, HttpClient, Lang};
pub use output::{GeneratedFile, GeneratedOutput, GenerationManifest, MANIFEST_FILE};
