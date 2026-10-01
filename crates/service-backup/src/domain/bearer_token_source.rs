//! The port the admin snapshot transport reads a bearer token from before
//! each request.

use super::BearerTokenError;

/// A bearer token that may change between requests (a rotated, projected
/// ServiceAccount token, for example).
///
/// The transport asks once per request and never caches the answer. The
/// token is handed straight to the request's `Authorization` header; the
/// transport reports a failure without the token or the adapter's error.
pub trait BearerTokenSource: Send + Sync {
    /// The token to send now.
    fn bearer_token(&self) -> Result<String, BearerTokenError>;
}
