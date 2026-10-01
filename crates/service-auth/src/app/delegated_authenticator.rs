//! `DelegatedAuthenticator::new`, wired to the system clock.

use std::sync::Arc;

use crate::application::k8s::DelegatedAuthenticator;
use crate::domain::k8s::{DelegatedAuthConfig, ReviewBackend};
use crate::infrastructure::k8s::SystemClock;

impl DelegatedAuthenticator {
    /// An authenticator on the system clock. See
    /// [`with_clock`](Self::with_clock) to inject one.
    pub fn new(backend: Arc<dyn ReviewBackend>, config: DelegatedAuthConfig) -> Self {
        Self::with_clock(backend, config, Arc::new(SystemClock))
    }
}
