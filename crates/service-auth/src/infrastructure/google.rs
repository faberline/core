//! Google adapters: the HTTPS JWKS and tokeninfo clients, the metadata
//! server ID-token source, and the system clock.

mod http;
mod metadata;
mod system_clock;

pub use http::{HttpAccessTokenIntrospection, HttpJwksSource};
pub use metadata::{MetadataTokenError, MetadataTokenSource};
pub use system_clock::SystemClock;
