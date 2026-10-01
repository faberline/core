//! The token model: a [`Scope`], and signing and verifying it.

mod claim_token;
mod hmac;
mod ids;
mod scope;

pub use claim_token::{sign, verify};
pub use ids::{ExpiryUnixSeconds, InputKey, ResultKey};
pub use scope::Scope;
