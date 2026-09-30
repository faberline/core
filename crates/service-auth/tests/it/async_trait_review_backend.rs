//! `ReviewBackend` is written by hand in the shape `#[async_trait]` expands
//! to, so downstream implementations written with `#[async_trait]` keep
//! compiling. This one is written the way lumen writes its test cluster
//! (src/auth.rs) and is driven through the public `DelegatedAuthenticator`.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use service_auth::k8s::{
    AccessReviewOutcome, DelegatedAuthConfig, DelegatedAuthError, DelegatedAuthenticator,
    ResourceAttributes, ReviewBackend, ReviewError, ReviewedIdentity, TokenReviewOutcome,
};

struct Cluster {
    username: String,
    audiences: Vec<String>,
    reachable: bool,
    asked: Mutex<Vec<ResourceAttributes>>,
}

#[async_trait]
impl ReviewBackend for Cluster {
    async fn review_token(
        &self,
        _token: &str,
        _audiences: &[String],
    ) -> Result<TokenReviewOutcome, ReviewError> {
        if !self.reachable {
            return Err(ReviewError::Transport("apiserver unreachable".into()));
        }
        Ok(TokenReviewOutcome::authenticated(
            ReviewedIdentity {
                username: self.username.clone(),
                uid: "uid-1".into(),
                groups: vec!["system:serviceaccounts".into()],
                ..Default::default()
            },
            self.audiences.clone(),
        ))
    }

    async fn review_access(
        &self,
        identity: &ReviewedIdentity,
        attributes: &ResourceAttributes,
    ) -> Result<AccessReviewOutcome, ReviewError> {
        self.asked.lock().unwrap().push(attributes.clone());
        Ok(
            if identity.username == self.username && attributes.verb == "get" {
                AccessReviewOutcome::allow()
            } else {
                AccessReviewOutcome::deny("no RoleBinding grants this")
            },
        )
    }
}

fn cluster(reachable: bool) -> Arc<Cluster> {
    Arc::new(Cluster {
        username: "system:serviceaccount:apps:api".into(),
        audiences: vec!["serving".into()],
        reachable,
        asked: Mutex::new(Vec::new()),
    })
}

fn authenticator(backend: Arc<Cluster>) -> DelegatedAuthenticator {
    let config = DelegatedAuthConfig::new(vec!["serving".into()]).unwrap();
    DelegatedAuthenticator::new(backend, config)
}

fn widgets(verb: &str) -> ResourceAttributes {
    ResourceAttributes::new("example.test", "serving", "widgets", None, verb)
}

#[tokio::test]
async fn an_async_trait_backend_authenticates_and_authorizes() {
    let backend = cluster(true);
    let auth = authenticator(backend.clone());

    let principal = auth.authenticate("token").await.unwrap();
    assert_eq!(principal.namespace(), "apps");
    assert_eq!(principal.name(), "api");

    auth.authorize(&principal, &widgets("get")).await.unwrap();
    let denied = auth.authorize(&principal, &widgets("delete")).await;
    assert!(matches!(denied, Err(DelegatedAuthError::Denied(_))));
    assert_eq!(backend.asked.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn an_async_trait_backend_error_is_unavailable() {
    let auth = authenticator(cluster(false));
    let error = auth.authenticate("token").await.unwrap_err();
    assert!(matches!(error, DelegatedAuthError::Unavailable(_)));
    assert_eq!(
        error.to_string(),
        DelegatedAuthError::Unavailable(ReviewError::Transport("apiserver unreachable".into()))
            .to_string()
    );
}

#[test]
fn an_async_trait_backend_is_a_dyn_review_backend() {
    let backend: Arc<dyn ReviewBackend> = cluster(true);
    let _ = DelegatedAuthenticator::new(
        backend,
        DelegatedAuthConfig::new(vec!["serving".into()]).unwrap(),
    );
}
