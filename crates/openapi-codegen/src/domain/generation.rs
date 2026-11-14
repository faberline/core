//! What a generation run is asked for and what it produces: the target
//! language, the TypeScript HTTP backend, the file-backed bearer-token and
//! Managed auth opt-ins, the in-memory generated files, and the contract
//! manifest.

mod file_bearer_auth;
mod file_bearer_auth_error;
mod generation_auth;
mod generation_auth_error;
mod managed_auth;
mod managed_auth_error;
mod operation_selection;
mod options;
mod output;
mod service_compatibility_error;

pub use file_bearer_auth::{FileBearerAuth, FileBearerScheme};
pub use file_bearer_auth_error::FileBearerAuthError;
pub use generation_auth::GenerationAuth;
pub use generation_auth_error::GenerationAuthError;
pub use managed_auth::{ManagedAuth, MANAGED_TOKEN_PATH};
pub use managed_auth_error::ManagedAuthError;
pub use operation_selection::{OperationSelection, OperationSelectionError};
pub use options::{GenOptions, HttpClient, Lang};
pub use output::{GeneratedFile, GeneratedOutput, GenerationManifest, MANIFEST_FILE};
pub use service_compatibility_error::ServiceCompatibilityError;
