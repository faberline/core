//! `TokenSource::new`, wired to the system clock.

use std::sync::Arc;

use crate::application::k8s::{TokenMinter, TokenRequestTarget, TokenSource};
use crate::infrastructure::k8s::SystemClock;

impl TokenSource {
    /// A token source on the system clock. See
    /// [`with_clock`](Self::with_clock) to inject one.
    pub fn new(minter: Arc<dyn TokenMinter>, target: TokenRequestTarget) -> Self {
        Self::with_clock(minter, target, Arc::new(SystemClock))
    }
}
