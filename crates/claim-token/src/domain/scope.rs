use serde::{Deserialize, Serialize};

use super::{ExpiryUnixSeconds, InputKey, ResultKey};

/// What a token authorizes. `Scope::new` builds it; a verified token's
/// scope comes from `Deserialize`, which fills the fields directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    /// Readable input key (claim-check GET /v1/inputs/{r}).
    r: InputKey,
    /// Writable result key (claim-check PUT /v1/results/{w}).
    w: ResultKey,
    /// Expiry, unix seconds.
    exp: ExpiryUnixSeconds,
}

impl Scope {
    /// A scope that reads `r`, writes `w` and expires at `exp` (unix seconds).
    /// The key types prevent a caller from swapping read and write access.
    ///
    /// ```compile_fail
    /// use claim_token::{ExpiryUnixSeconds, InputKey, ResultKey, Scope};
    /// Scope::new(ResultKey::new("out"), InputKey::new("in"), ExpiryUnixSeconds::new(1));
    /// ```
    pub fn new(r: InputKey, w: ResultKey, exp: ExpiryUnixSeconds) -> Self {
        Self { r, w, exp }
    }

    /// Readable input key (claim-check GET /v1/inputs/{r}).
    pub fn r(&self) -> &InputKey {
        &self.r
    }

    /// Writable result key (claim-check PUT /v1/results/{w}).
    pub fn w(&self) -> &ResultKey {
        &self.w
    }

    /// Expiry, unix seconds.
    pub fn exp(&self) -> ExpiryUnixSeconds {
        self.exp
    }
}

#[cfg(test)]
mod tests;
