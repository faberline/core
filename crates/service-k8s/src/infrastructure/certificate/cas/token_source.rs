//! Bearer tokens for CA Service: the GKE metadata server and workload
//! identity federation.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use serde_json::Value;

use super::{AccessTokenSource, GKE_METADATA_TOKEN_ENDPOINT};
use crate::domain::certificate::issuer::IssuerError;

/// Fetches a short-lived OAuth access token from the GKE Workload Identity metadata server.
///
/// Endpoint: `http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token`
/// Header: `Metadata-Flavor: Google`
pub struct GkeMetadataTokenSource {
    endpoint: String,
    client: reqwest::Client,
    cached: Mutex<Option<(String, Instant)>>,
}

impl GkeMetadataTokenSource {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client for GkeMetadataTokenSource");

        Self {
            endpoint: GKE_METADATA_TOKEN_ENDPOINT.to_string(),
            client,
            cached: Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[cfg(test)]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    #[cfg(test)]
    pub fn with_cached_token(self, token: impl Into<String>, expiry: Instant) -> Self {
        if let Ok(mut guard) = self.cached.lock() {
            *guard = Some((token.into(), expiry));
        }
        self
    }
}

impl Default for GkeMetadataTokenSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessTokenSource for GkeMetadataTokenSource {
    fn token<'a>(&'a self) -> BoxFuture<'a, Result<String, IssuerError>> {
        Box::pin(async move {
            let now = Instant::now();
            if let Ok(guard) = self.cached.lock() {
                if let Some((ref tok, expiry)) = *guard {
                    if now < expiry {
                        return Ok(tok.clone());
                    }
                }
            }

            let resp = self
                .client
                .get(&self.endpoint)
                .header("Metadata-Flavor", "Google")
                .send()
                .await
                .map_err(|err| {
                    IssuerError::Upstream(format!(
                        "metadata server request failed: status {}",
                        err.status()
                            .map_or("network error".to_string(), |s| s.to_string())
                    ))
                })?;

            let status = resp.status();
            if !status.is_success() {
                return Err(IssuerError::Upstream(format!(
                    "metadata server returned HTTP status {status}"
                )));
            }

            let body: serde_json::Value = resp.json().await.map_err(|_| {
                IssuerError::Upstream("metadata server response is not valid JSON".to_string())
            })?;

            let access_token = body
                .get("access_token")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    IssuerError::Upstream(
                        "metadata server response missing non-empty access_token".to_string(),
                    )
                })?
                .to_string();

            let expires_in_secs =
                body.get("expires_in")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        IssuerError::Upstream(
                            "metadata server response missing valid positive integer expires_in"
                                .to_string(),
                        )
                    })?;

            if expires_in_secs == 0 {
                return Err(IssuerError::Upstream(
                    "metadata server returned zero token lifetime".to_string(),
                ));
            }

            if expires_in_secs > 300 {
                let refresh_secs = expires_in_secs - 300;
                let expiry = now + Duration::from_secs(refresh_secs);
                if let Ok(mut guard) = self.cached.lock() {
                    *guard = Some((access_token.clone(), expiry));
                }
            } else if let Ok(mut guard) = self.cached.lock() {
                *guard = None;
            }

            Ok(access_token)
        })
    }
}

/// Exchanges the kubelet's projected KSA token for a federated access token.
///
/// The projected token is re-read on every exchange rather than cached from
/// startup. The kubelet rotates it, and a controller that read it once would
/// keep presenting an expired assertion until it restarted — the classic way a
/// "short-lived credentials" design ends up with a long-lived one.
pub struct WorkloadIdentityTokenSource {
    /// Path the projected token volume is mounted at.
    token_path: PathBuf,
    /// STS audience, as rendered by the installation (#3109).
    audience: String,
    /// Scope requested for the exchanged token.
    scope: String,
    sts_endpoint: String,
    client: reqwest::Client,
    /// Last exchanged token and when it stops being usable.
    cached: Mutex<Option<(String, Instant)>>,
}

/// Federated tokens are minted for an hour; refreshing a few minutes early
/// avoids handing an about-to-expire token to a request that then takes longer
/// than the remainder.
const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(300);

impl WorkloadIdentityTokenSource {
    pub fn new(token_path: impl Into<PathBuf>, audience: impl Into<String>) -> Self {
        Self {
            token_path: token_path.into(),
            audience: audience.into(),
            scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
            sts_endpoint: "https://sts.googleapis.com/v1/token".to_string(),
            client: reqwest::Client::new(),
            cached: Mutex::new(None),
        }
    }

    /// The form body of the exchange, as a list of pairs.
    ///
    /// Split out so the shape is assertable without a network: the audience and
    /// the subject-token type are the two fields that decide whether STS will
    /// accept a KSA assertion at all, and getting either wrong fails at runtime
    /// with a message that names neither.
    pub fn exchange_form(&self, subject_token: &str) -> Vec<(&'static str, String)> {
        vec![
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange".to_string(),
            ),
            ("audience", self.audience.clone()),
            ("scope", self.scope.clone()),
            (
                "requested_token_type",
                "urn:ietf:params:oauth:token-type:access_token".to_string(),
            ),
            ("subject_token", subject_token.to_string()),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:jwt".to_string(),
            ),
        ]
    }

    async fn exchange(&self) -> Result<String, IssuerError> {
        if let Some((token, expires_at)) = self.cached.lock().expect("token cache").clone() {
            if Instant::now() + TOKEN_REFRESH_MARGIN < expires_at {
                return Ok(token);
            }
        }

        let assertion = tokio::fs::read_to_string(&self.token_path)
            .await
            .map_err(|err| {
                // The path, never the contents.
                IssuerError::Upstream(format!(
                    "read projected token at {}: {err}",
                    self.token_path.display()
                ))
            })?;

        let form = self.exchange_form(assertion.trim());
        let response = self
            .client
            .post(&self.sts_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|err| IssuerError::Upstream(format!("token exchange: {err}")))?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|err| IssuerError::Upstream(format!("token exchange response: {err}")))?;
        if !status.is_success() {
            // Deliberately not the body: a failed exchange commonly echoes the
            // assertion back in its error detail.
            return Err(IssuerError::Upstream(format!(
                "token exchange rejected with {status}"
            )));
        }
        let token = body["access_token"]
            .as_str()
            .ok_or_else(|| IssuerError::Upstream("token exchange returned no token".into()))?
            .to_string();
        let lifetime = Duration::from_secs(body["expires_in"].as_u64().unwrap_or(3600));
        *self.cached.lock().expect("token cache") =
            Some((token.clone(), Instant::now() + lifetime));
        Ok(token)
    }
}

impl AccessTokenSource for WorkloadIdentityTokenSource {
    fn token<'a>(&'a self) -> BoxFuture<'a, Result<String, IssuerError>> {
        Box::pin(self.exchange())
    }
}
