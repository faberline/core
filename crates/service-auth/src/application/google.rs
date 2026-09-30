//! The Google verifier: ID-token and access-token paths over the injectable
//! JWKS port (here) and the introspection port (domain), the bounded-rate key
//! cache, and credential classification.

mod credential;
mod jwks_cache;
mod jwks_source;
mod verifier;

pub use credential::{classify, Credential};
pub use jwks_cache::JwksCache;
pub use jwks_source::JwksSource;
pub use verifier::GoogleVerifier;
