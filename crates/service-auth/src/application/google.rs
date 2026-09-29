//! The Google verifier: ID-token and access-token paths over injectable JWKS
//! and introspection ports, the bounded-rate key cache, and credential
//! classification.

mod credential;
mod introspection;
mod jwks_cache;
mod jwks_source;
mod verifier;

pub use credential::{classify, Credential};
pub use introspection::{AccessTokenIntrospection, IntrospectedToken};
pub use jwks_cache::JwksCache;
pub use jwks_source::JwksSource;
pub use verifier::GoogleVerifier;
