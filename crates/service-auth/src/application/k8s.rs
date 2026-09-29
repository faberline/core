//! Kubernetes delegated auth: the TokenReview/SubjectAccessReview port, the
//! caching authenticator and its metrics, and the TokenRequest token source.

mod authenticator;
mod delegated_error;
mod metrics;
mod review;
mod system_clock;
mod token_request;

use crate::domain::k8s::principal;

pub use authenticator::DelegatedAuthenticator;
pub use delegated_error::DelegatedAuthError;
pub use metrics::DelegatedAuthMetrics;
pub use review::{
    AccessReviewOutcome, ExtraFields, ResourceAttributes, ReviewBackend, ReviewError,
    TokenReviewOutcome,
};
pub use system_clock::SystemClock;
pub use token_request::{
    MintedToken, TokenMinter, TokenRequestError, TokenRequestTarget, TokenSource,
    DEFAULT_EXPIRATION_SECONDS, MIN_EXPIRATION_SECONDS,
};
