use async_trait::async_trait;

use super::SystemClock;
use crate::application::k8s::{MintedToken, TokenMinter, TokenRequestError, TokenRequestTarget};
use crate::domain::k8s::Clock;

// ---------------------------------------------------------------------------
// The transport
// ---------------------------------------------------------------------------

/// The kube-rs implementation. The only thing in this module that opens a
/// socket.
///
/// Its client comes from the ambient configuration — in-cluster when there is
/// a mounted ServiceAccount, the kubeconfig otherwise, including exec
/// credential plugins such as GKE's. Those plugins are how a cloud identity
/// reaches kube-apiserver, and they are the *only* way one is used here: this
/// crate links no cloud SDK, reads no application-default credential, and
/// never speaks to a metadata server. `kube`'s deprecated in-tree `gcp`
/// auth-provider path is likewise absent, because the `oauth` feature that
/// would enable it is off — a kubeconfig still using it fails here rather than
/// quietly acquiring a Google token in-process.
#[cfg(feature = "k8s")]
pub struct KubeTokenMinter {
    client: kube::Client,
}

#[cfg(feature = "k8s")]
impl KubeTokenMinter {
    /// Build from the ambient Kubernetes configuration.
    pub async fn from_ambient_config() -> Result<Self, TokenRequestError> {
        let client =
            kube::Client::try_default()
                .await
                .map_err(|error| TokenRequestError::NoIdentity {
                    detail: error.to_string(),
                })?;
        Ok(Self { client })
    }

    /// Build from a named kubeconfig context, or from the ambient
    /// configuration when `context` is `None`.
    ///
    /// Naming a context matters here in a way it does not for a serving
    /// process: the whole point of this call is that the caller chooses which
    /// identity is asking, and a machine with several clusters configured
    /// should not have that decided by whichever `kubectl config use-context`
    /// ran last.
    pub async fn from_context(context: Option<&str>) -> Result<Self, TokenRequestError> {
        let Some(context) = context else {
            return Self::from_ambient_config().await;
        };
        let options = kube::config::KubeConfigOptions {
            context: Some(context.to_string()),
            cluster: None,
            user: None,
        };
        let config = kube::Config::from_kubeconfig(&options).await.map_err(|e| {
            TokenRequestError::NoIdentity {
                detail: format!("kubeconfig context `{context}`: {e}"),
            }
        })?;
        let client = kube::Client::try_from(config).map_err(|e| TokenRequestError::NoIdentity {
            detail: format!("kubeconfig context `{context}`: {e}"),
        })?;
        Ok(Self { client })
    }

    pub fn from_client(client: kube::Client) -> Self {
        Self { client }
    }

    /// What the apiserver says this client is, via `SelfSubjectReview` — the
    /// call behind `kubectl auth whoami`.
    ///
    /// Asked only to name the caller in a denial. Scraping the username out of
    /// the RBAC message would work today and is exactly the kind of thing that
    /// stops working in a version bump, silently, in the error path nobody
    /// tests.
    pub async fn whoami(&self) -> Option<String> {
        use k8s_openapi::api::authentication::v1::SelfSubjectReview;
        let api: kube::Api<SelfSubjectReview> = kube::Api::all(self.client.clone());
        let review = api
            .create(&kube::api::PostParams::default(), &Default::default())
            .await
            .ok()?;
        review.status?.user_info?.username
    }
}

#[cfg(feature = "k8s")]
#[async_trait]
impl TokenMinter for KubeTokenMinter {
    async fn mint(&self, target: &TokenRequestTarget) -> Result<MintedToken, TokenRequestError> {
        use k8s_openapi::api::authentication::v1::TokenRequest;
        use k8s_openapi::api::core::v1::ServiceAccount;

        // The body a test can assert on, deserialized into the typed request
        // rather than rebuilt beside it.
        let request: TokenRequest = serde_json::from_value(target.request_body()).map_err(|e| {
            TokenRequestError::Malformed {
                detail: format!("could not build the TokenRequest body: {e}"),
            }
        })?;

        let api: kube::Api<ServiceAccount> =
            kube::Api::namespaced(self.client.clone(), target.namespace());
        let response = match api
            .create_token_request(
                target.service_account(),
                &kube::api::PostParams::default(),
                &request,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => return Err(self.classify(target, error).await),
        };

        let Some(status) = response.status else {
            return Err(TokenRequestError::Malformed {
                detail: "TokenRequest response carried no status".to_string(),
            });
        };
        if status.token.is_empty() {
            return Err(TokenRequestError::Malformed {
                detail: "TokenRequest response carried an empty token".to_string(),
            });
        }
        let expires_at_millis = status
            .expiration_timestamp
            .0
            .timestamp_millis()
            .max(0)
            .unsigned_abs();
        let now = SystemClock.now_millis();
        Ok(MintedToken::new(status.token, now, expires_at_millis))
    }
}

#[cfg(feature = "k8s")]
impl KubeTokenMinter {
    /// Translate a `kube` error, resolving the caller's username for the one
    /// case where naming it is the whole remediation.
    async fn classify(&self, target: &TokenRequestTarget, error: kube::Error) -> TokenRequestError {
        match error {
            kube::Error::Api(response) if response.code == 403 => TokenRequestError::Forbidden {
                username: self.whoami().await,
                namespace: target.namespace().to_string(),
                service_account: target.service_account().to_string(),
                detail: response.message,
            },
            kube::Error::Api(response) if response.code == 404 => {
                TokenRequestError::NoSuchServiceAccount {
                    namespace: target.namespace().to_string(),
                    service_account: target.service_account().to_string(),
                }
            }
            kube::Error::Api(response) if response.code == 401 => TokenRequestError::NoIdentity {
                detail: response.message,
            },
            kube::Error::Api(response) => TokenRequestError::Transport {
                detail: format!("apiserver returned {}: {}", response.code, response.message),
            },
            other => TokenRequestError::Transport {
                detail: other.to_string(),
            },
        }
    }
}
