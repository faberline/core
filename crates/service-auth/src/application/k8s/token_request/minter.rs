use async_trait::async_trait;

use super::error::TokenRequestError;
use super::minted::MintedToken;
use super::target::TokenRequestTarget;

// ---------------------------------------------------------------------------
// The seam
// ---------------------------------------------------------------------------

/// Whatever can turn a [`TokenRequestTarget`] into a [`MintedToken`].
///
/// One real implementation ([`KubeTokenMinter`]) and, in tests, a fake — which
/// is the point. The behaviour worth testing here is what happens over the
/// course of an hour, and the alternative to a seam is a test suite that takes
/// one.
#[async_trait]
pub trait TokenMinter: Send + Sync {
    async fn mint(&self, target: &TokenRequestTarget) -> Result<MintedToken, TokenRequestError>;
}
