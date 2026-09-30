//! Google identity verification — "IAM decides who you are, the product
//! decides what you may do".
//!
//! This module resolves a Google-issued credential to a **verified email** and
//! then hands that email to the unmodified role map. [`Role`], [`covers`], and
//! [`ensure`] are untouched: authentication gains a new source, authorization
//! does not change.
//!
//! ## Two credentials, two verification models
//!
//! They are different artifacts from different endpoints, and conflating them
//! produces a 401 that looks like a service bug:
//!
//! | | ID token | Access token |
//! |---|---|---|
//! | Shape | RS256 JWT, three segments, carries `kid` | opaque `ya29.*` |
//! | Verified by | offline, against Google's published JWKS | calling Google's introspection endpoint |
//! | `aud` | bound to a requested audience | the client ID that minted it |
//! | Mintable by a plain user account | **no** | yes |
//! | Mintable by a service account | yes | yes |
//!
//! The last two rows are why both paths exist rather than one. A developer
//! cannot mint a custom-audience ID token from a plain Google account, and a
//! workload should not put a Google round trip in its request path. So a
//! service takes the offline path and a human takes the introspection path —
//! the same split Cloud SQL IAM Database Authentication uses.
//!
//! ## Which path a credential takes
//!
//! Selection is by shape, never by trying each in turn ([`classify`]):
//!
//! ```text
//! three dot-separated segments, RS256, carries kid  ->  offline JWKS path
//! anything else                                     ->  registry lookup,
//!                                                       then introspection
//! ```
//!
//! Registry-first on the opaque branch is deliberate: a pre-shared bearer
//! secret is a local map hit, and it must not acquire a network round trip
//! just because Google identities became possible.
//!
//! ## Staying off the network
//!
//! Steady state makes no upstream call. JWKS keys are cached and only
//! refetched when a `kid` misses — Google rotates signing keys, so pinning one
//! would mean a total outage at rotation — and that refetch is rate-limited so
//! fabricated `kid` values cannot be amplified into a flood against Google.
//! Introspection results are cached for `min(expires_in, ceiling)`.
//!
//! ## Reachability is not rejection
//!
//! [`GoogleAuthError::IntrospectionUnavailable`] and
//! [`GoogleAuthError::SigningKeyUnavailable`] surface as
//! [`AuthError::Unavailable`] (503), never as 401. An upstream outage that
//! presents to the caller as "your credential is invalid" is an unfixable
//! support call.
//!
//! [`Role`]: crate::Role
//! [`covers`]: crate::Role::covers
//! [`ensure`]: crate::RoleMapPrincipal::ensure

pub use crate::application::google::{classify, Credential, GoogleVerifier, JwksCache, JwksSource};
pub use crate::domain::google::{
    AccessTokenIntrospection, Clock, GoogleAuthConfig, GoogleAuthError, IntrospectedToken,
    InvalidReason, DEFAULT_INTROSPECTION_TTL_CEILING, DEFAULT_JWKS_REFETCH_MIN_INTERVAL,
    GOOGLE_ISSUERS, GOOGLE_JWKS_URL, GOOGLE_TOKENINFO_URL,
};
pub use crate::infrastructure::google::{
    HttpAccessTokenIntrospection, HttpJwksSource, SystemClock,
};
