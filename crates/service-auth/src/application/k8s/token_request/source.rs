use std::sync::Arc;

use tokio::sync::Mutex;

use super::error::TokenRequestError;
use super::minted::MintedToken;
use super::minter::TokenMinter;
use super::target::TokenRequestTarget;
use crate::domain::k8s::{Clock, ProjectedToken};

/// A token that keeps itself current.
///
/// Holds exactly one token at a time, in memory, and hands out clones. There
/// is no file, no environment variable, and no way to read it out other than
/// [`ProjectedToken::expose`] — which the one place that writes an
/// `Authorization` header calls, and nothing else does.
pub struct TokenSource {
    minter: Arc<dyn TokenMinter>,
    target: TokenRequestTarget,
    clock: Arc<dyn Clock>,
    current: Mutex<Option<MintedToken>>,
}

impl TokenSource {
    pub fn with_clock(
        minter: Arc<dyn TokenMinter>,
        target: TokenRequestTarget,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            minter,
            target,
            clock,
            current: Mutex::new(None),
        }
    }

    pub fn target(&self) -> &TokenRequestTarget {
        &self.target
    }

    /// A token that is good right now, minting one if the one in hand is not.
    ///
    /// A failed refresh is returned, not swallowed. The tempting alternative —
    /// keep serving the old token until it actually expires — turns a revoked
    /// grant into a delay instead of a refusal, and the whole reason these
    /// tokens are short is so that revocation means something.
    pub async fn token(&self) -> Result<ProjectedToken, TokenRequestError> {
        let mut current = self.current.lock().await;
        let now = self.clock.now_millis();
        if let Some(token) = current.as_ref() {
            if now < token.refresh_at_millis() {
                return Ok(token.token().clone());
            }
        }
        let minted = self.minter.mint(&self.target).await?;
        let token = minted.token().clone();
        *current = Some(minted);
        Ok(token)
    }
}
