//! Delegating both halves of the question to kube-apiserver.
//!
//! A service that adopts this stops having an opinion about who its callers
//! are. `TokenReview` answers "whose token is this?", `SubjectAccessReview`
//! answers "may they do this?", and everything in between — audience checking,
//! ServiceAccount-only admission, caching, and what to do when the apiserver
//! is unreachable — is the policy in this module.
//!
//! Three properties are worth stating up front, because they are the ones a
//! reader should be able to check:
//!
//! - **Nothing here can produce an allow that the apiserver did not.** The only
//!   sources of a positive answer are a live review and a cache entry that a
//!   live review put there. There is no configuration, no default, and no
//!   error path that yields access.
//! - **A raw token is never stored, logged, or embedded in an error.** The
//!   cache is keyed by a SHA-256 digest of the token, and the only
//!   token-derived value that escapes is a truncated [`fingerprint`], which
//!   correlates audit lines without being a credential.
//! - **The outage path is bounded and one-directional.** When a review fails,
//!   an already-cached answer may be reused for [`CachePolicy::stale_window`]
//!   past its TTL — and then never again. "The apiserver is down" is not a
//!   reason to keep serving.
//!
//! This module knows nothing about any service's resources. It is handed
//! [`ResourceAttributes`] and returns a verdict; naming what a resource *is* is
//! the calling service's job.

pub use crate::application::k8s::{
    DelegatedAuthError, DelegatedAuthMetrics, DelegatedAuthenticator,
};
pub use crate::domain::k8s::{fingerprint, AuthRejection, DelegatedAuthConfig, MissingAudience};
