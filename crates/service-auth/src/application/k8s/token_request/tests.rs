use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use super::*;
use crate::k8s::cache::{Clock, ManualClock};

const AUDIENCE: &str = "callee.example.com";
const CANARY: &str = "canary-minted-token-must-never-be-printed";

fn target() -> TokenRequestTarget {
    TokenRequestTarget::new("ops", "app-client", AUDIENCE).expect("a valid target")
}

/// Mints a distinct token per call and records what it was asked for, so a
/// test can assert both the request and how many were made.
struct RecordingMinter {
    lifetime_millis: u64,
    calls: AtomicU64,
    clock: Arc<ManualClock>,
    fail_after: u64,
}

impl RecordingMinter {
    fn new(clock: Arc<ManualClock>, lifetime: Duration) -> Self {
        Self {
            lifetime_millis: lifetime.as_millis() as u64,
            calls: AtomicU64::new(0),
            clock,
            fail_after: u64::MAX,
        }
    }

    fn failing_after(mut self, calls: u64) -> Self {
        self.fail_after = calls;
        self
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl TokenMinter for RecordingMinter {
    async fn mint(&self, target: &TokenRequestTarget) -> Result<MintedToken, TokenRequestError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n >= self.fail_after {
            return Err(TokenRequestError::Forbidden {
                username: Some("alice@example.com".to_string()),
                namespace: target.namespace().to_string(),
                service_account: target.service_account().to_string(),
                detail: "the grant was removed".to_string(),
            });
        }
        let now = self.clock.now_millis();
        Ok(MintedToken::new(
            format!("{CANARY}-{n}"),
            now,
            now + self.lifetime_millis,
        ))
    }
}

mod errors;
mod refresh;
mod request;
