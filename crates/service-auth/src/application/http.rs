//! The request-auth seam: the sync and async verifier traits, the bearer
//! extraction helper and middleware, and the shared rejection type.

mod async_verifier;
mod error;
mod middleware;
mod verifier;

pub use async_verifier::{async_auth_middleware, AsAsync, AsyncVerifier};
pub use error::AuthError;
pub use middleware::{auth_middleware, bearer_token};
pub use verifier::Verifier;
