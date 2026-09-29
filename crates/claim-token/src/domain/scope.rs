use serde::{Deserialize, Serialize};

/// What a token authorizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    /// Readable input key (claim-check GET /v1/inputs/{r}).
    pub r: String,
    /// Writable result key (claim-check PUT /v1/results/{w}).
    pub w: String,
    /// Expiry, unix seconds.
    pub exp: u64,
}
