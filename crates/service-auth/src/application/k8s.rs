//! Kubernetes delegated auth: the caching authenticator over the domain
//! TokenReview/SubjectAccessReview port, its metrics, and the TokenRequest
//! token source.

mod authenticator;
mod delegated_error;
mod metrics;
mod token_request;

use crate::domain::k8s::principal;

pub use authenticator::DelegatedAuthenticator;
pub use delegated_error::DelegatedAuthError;
pub use metrics::DelegatedAuthMetrics;
pub use token_request::{
    MintedToken, TokenMinter, TokenRequestError, TokenRequestTarget, TokenSource,
    DEFAULT_EXPIRATION_SECONDS, MIN_EXPIRATION_SECONDS,
};
