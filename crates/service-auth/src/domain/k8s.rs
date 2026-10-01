//! Kubernetes identity values: the reviewed ServiceAccount principal, the
//! decision cache and its millisecond clock port, token fingerprints, the
//! delegated-auth configuration, and the redacting token wrapper.

mod cache;
mod delegated_config;
pub(crate) mod principal;
mod projected_token;
mod rejection;
mod token_digest;

pub use cache::{CacheOutcome, CachePolicy, Clock, ManualClock, TtlCache};
pub use delegated_config::{DelegatedAuthConfig, MissingAudience};
pub use principal::{
    PrincipalRejection, ReviewedIdentity, ServiceAccountPrincipal, ServiceAccountRef,
    SERVICE_ACCOUNT_PREFIX,
};
pub use projected_token::ProjectedToken;
pub use rejection::AuthRejection;
pub use token_digest::fingerprint;
pub(crate) use token_digest::{digest, TokenDigest};
