//! TokenRequest: the validated target, the minted token and its refresh
//! point, the failure taxonomy, the minting port, and the self-refreshing
//! token source.

mod error;
mod minted;
mod minter;
mod source;
mod target;

pub use error::TokenRequestError;
pub use minted::MintedToken;
pub use minter::TokenMinter;
pub use source::TokenSource;
pub use target::{TokenRequestTarget, DEFAULT_EXPIRATION_SECONDS, MIN_EXPIRATION_SECONDS};

#[cfg(test)]
mod tests;
