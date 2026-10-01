//! Google identity values: the pinned endpoints and tunables, the
//! verification error taxonomy, the seconds-resolution clock port, and the
//! access-token introspection port with its answer.

mod clock;
mod config;
mod error;
mod introspection;

pub use clock::Clock;
pub use config::{
    GoogleAuthConfig, DEFAULT_INTROSPECTION_TTL_CEILING, DEFAULT_JWKS_REFETCH_MIN_INTERVAL,
    GOOGLE_ISSUERS, GOOGLE_JWKS_URL, GOOGLE_TOKENINFO_URL,
};
pub use error::{GoogleAuthError, InvalidReason};
pub(crate) use introspection::lenient_bool;
pub use introspection::{AccessTokenIntrospection, IntrospectedToken};
