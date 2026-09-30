use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;

use super::*;
use crate::k8s::cache::ManualClock;
use crate::k8s::delegated::*;
use crate::k8s::principal::ReviewedIdentity;
use crate::k8s::review::{AccessReviewOutcome, TokenReviewOutcome};
use crate::AuthError;

/// The audience this test suite's fictional service asks for. Deliberately
/// not any real service's: AC8 requires the shared library's tests to know
/// no product's resource strings.
const AUDIENCE: &str = "reviews.example.test";

/// A backend a test drives directly: it records what it was asked and
/// returns whatever the test queued.
#[derive(Default)]
struct ScriptedBackend {
    token: Mutex<Option<Result<TokenReviewOutcome, ReviewError>>>,
    access: Mutex<Option<Result<AccessReviewOutcome, ReviewError>>>,
    token_calls: Mutex<Vec<Vec<String>>>,
    access_calls: Mutex<Vec<(ReviewedIdentity, ResourceAttributes)>>,
    /// Every token the backend was handed, so a test can assert none of
    /// them reached anywhere they should not have.
    seen_tokens: Mutex<Vec<String>>,
}

impl ScriptedBackend {
    fn with_token(self, outcome: Result<TokenReviewOutcome, ReviewError>) -> Self {
        *self.token.lock().unwrap() = Some(outcome);
        self
    }

    fn with_access(self, outcome: Result<AccessReviewOutcome, ReviewError>) -> Self {
        *self.access.lock().unwrap() = Some(outcome);
        self
    }

    fn set_token(&self, outcome: Result<TokenReviewOutcome, ReviewError>) {
        *self.token.lock().unwrap() = Some(outcome);
    }

    fn set_access(&self, outcome: Result<AccessReviewOutcome, ReviewError>) {
        *self.access.lock().unwrap() = Some(outcome);
    }

    fn token_call_count(&self) -> usize {
        self.token_calls.lock().unwrap().len()
    }

    fn access_call_count(&self) -> usize {
        self.access_calls.lock().unwrap().len()
    }
}

#[async_trait]
impl ReviewBackend for ScriptedBackend {
    async fn review_token(
        &self,
        token: &str,
        audiences: &[String],
    ) -> Result<TokenReviewOutcome, ReviewError> {
        self.seen_tokens.lock().unwrap().push(token.to_string());
        self.token_calls.lock().unwrap().push(audiences.to_vec());
        self.token
            .lock()
            .unwrap()
            .clone()
            .expect("test queued no TokenReview outcome")
    }

    async fn review_access(
        &self,
        identity: &ReviewedIdentity,
        attributes: &ResourceAttributes,
    ) -> Result<AccessReviewOutcome, ReviewError> {
        self.access_calls
            .lock()
            .unwrap()
            .push((identity.clone(), attributes.clone()));
        self.access
            .lock()
            .unwrap()
            .clone()
            .expect("test queued no SubjectAccessReview outcome")
    }
}

fn config() -> DelegatedAuthConfig {
    DelegatedAuthConfig::new(vec![AUDIENCE.to_string()]).unwrap()
}

fn reviewed_identity(username: &str) -> ReviewedIdentity {
    ReviewedIdentity {
        username: username.into(),
        uid: "uid-1".into(),
        groups: vec!["system:serviceaccounts".into()],
        extra: BTreeMap::new(),
    }
}

fn reviewed(username: &str, audiences: &[&str]) -> TokenReviewOutcome {
    TokenReviewOutcome::authenticated(
        reviewed_identity(username),
        audiences.iter().map(|a| a.to_string()).collect(),
    )
}

fn attributes() -> ResourceAttributes {
    ResourceAttributes::new(
        "example.test",
        "serving",
        "widgets",
        Some("blue".into()),
        "get",
    )
}

fn authenticator(backend: Arc<ScriptedBackend>, clock: Arc<ManualClock>) -> DelegatedAuthenticator {
    DelegatedAuthenticator::with_clock(backend, config(), clock)
}

fn fixture() -> (Arc<ScriptedBackend>, Arc<ManualClock>) {
    (
        Arc::new(ScriptedBackend::default()),
        Arc::new(ManualClock::new(1_000_000)),
    )
}

mod authentication;
mod edges;
mod outages_and_caching;
