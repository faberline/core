use serde::{Deserialize, Serialize};

/// What a token authorizes. `Scope::new` builds it; a verified token's
/// scope comes from `Deserialize`, which fills the fields directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    /// Readable input key (claim-check GET /v1/inputs/{r}).
    r: String,
    /// Writable result key (claim-check PUT /v1/results/{w}).
    w: String,
    /// Expiry, unix seconds.
    exp: u64,
}

impl Scope {
    /// A scope that reads `r`, writes `w` and expires at `exp` (unix seconds).
    pub fn new(r: impl Into<String>, w: impl Into<String>, exp: u64) -> Self {
        Self {
            r: r.into(),
            w: w.into(),
            exp,
        }
    }

    /// Readable input key (claim-check GET /v1/inputs/{r}).
    pub fn r(&self) -> &str {
        &self.r
    }

    /// Writable result key (claim-check PUT /v1/results/{w}).
    pub fn w(&self) -> &str {
        &self.w
    }

    /// Expiry, unix seconds.
    pub fn exp(&self) -> u64 {
        self.exp
    }
}

#[cfg(test)]
mod tests;
