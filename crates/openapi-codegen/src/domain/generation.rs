//! What a generation run is asked for and what it produces: the target
//! language, the TypeScript HTTP backend, the file-backed bearer-token
//! opt-in, the in-memory generated files, and the contract manifest.

mod file_bearer_auth;
mod file_bearer_auth_error;
mod options;
mod output;
mod service_compatibility_error;

pub use file_bearer_auth::{FileBearerAuth, FileBearerScheme};
pub use file_bearer_auth_error::FileBearerAuthError;
pub use options::{GenOptions, HttpClient, Lang};
pub use output::{GeneratedFile, GeneratedOutput, GenerationManifest, MANIFEST_FILE};
pub use service_compatibility_error::ServiceCompatibilityError;
