//! Google adapters: the HTTPS JWKS and tokeninfo clients and the system
//! clock.

mod http;
mod system_clock;

pub use http::{HttpAccessTokenIntrospection, HttpJwksSource};
pub use system_clock::SystemClock;
