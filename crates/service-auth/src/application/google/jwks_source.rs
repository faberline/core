use async_trait::async_trait;
use jsonwebtoken::jwk::JwkSet;

/// Where ID-token signing keys come from.
#[async_trait]
pub trait JwksSource: Send + Sync {
    /// Fetch the current key set. The error is a message, not a typed cause:
    /// every failure here means the same thing to a caller — upstream is not
    /// answering — and the detail exists only for the operator's log.
    async fn fetch(&self) -> Result<JwkSet, String>;
}
