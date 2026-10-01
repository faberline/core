//! Google identity values: the pinned endpoints and tunables, the
//! verification error taxonomy, and the seconds-resolution clock port.

mod clock;
mod config;
mod error;

pub use clock::Clock;
pub use config::{
    GoogleAuthConfig, DEFAULT_INTROSPECTION_TTL_CEILING, DEFAULT_JWKS_REFETCH_MIN_INTERVAL,
    DEFAULT_METADATA_BASE_URL, GOOGLE_ISSUERS, GOOGLE_JWKS_URL, GOOGLE_TOKENINFO_URL,
};
pub use error::{GoogleAuthError, InvalidReason};
