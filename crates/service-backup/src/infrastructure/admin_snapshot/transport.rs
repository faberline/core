use std::{fmt, sync::Arc};

use super::transport_config::AdminSnapshotTransportConfig;
use crate::domain::BearerTokenSource;

/// The standard operation performed by [`AdminSnapshotTransport`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSnapshotOperation {
    Backup,
    Restore,
}

/// Redacted errors from the strict admin snapshot transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSnapshotTransportError {
    ClientBuildFailed,
    RequestFailed {
        operation: AdminSnapshotOperation,
    },
    UnexpectedStatus {
        operation: AdminSnapshotOperation,
        status: reqwest::StatusCode,
    },
    ResponseReadFailed {
        operation: AdminSnapshotOperation,
    },
}

impl fmt::Display for AdminSnapshotTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientBuildFailed => f.write_str("admin snapshot client build failed"),
            Self::RequestFailed { operation } => {
                write!(f, "admin snapshot {operation:?} request failed")
            }
            Self::UnexpectedStatus { operation, status } => {
                write!(
                    f,
                    "admin snapshot {operation:?} returned unexpected status {status}"
                )
            }
            Self::ResponseReadFailed { operation } => {
                write!(f, "admin snapshot {operation:?} response read failed")
            }
        }
    }
}

impl std::error::Error for AdminSnapshotTransportError {}

impl AdminSnapshotOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backup => "backup",
            Self::Restore => "restore",
        }
    }
}

enum AdminCredential {
    None,
    Static(String),
    Projected(Arc<dyn BearerTokenSource>),
}

/// Per-request product policy for a shared admin snapshot call.
pub struct AdminSnapshotRequest {
    credential: AdminCredential,
    headers: reqwest::header::HeaderMap,
}

impl AdminSnapshotRequest {
    pub fn new() -> Self {
        Self {
            credential: AdminCredential::None,
            headers: reqwest::header::HeaderMap::new(),
        }
    }

    pub fn with_static_bearer(mut self, token: impl Into<String>) -> Self {
        self.credential = AdminCredential::Static(token.into());
        self
    }

    /// Ask `source` for the bearer immediately before every request, so a
    /// rotated token is observed. A failure is reported as
    /// [`AdminSnapshotRequestError::CredentialFailed`], without the token or
    /// the source's error.
    pub(crate) fn with_bearer_source(mut self, source: Arc<dyn BearerTokenSource>) -> Self {
        self.credential = AdminCredential::Projected(source);
        self
    }

    pub fn with_header(
        mut self,
        name: &str,
        value: &str,
    ) -> std::result::Result<Self, AdminSnapshotRequestError> {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| AdminSnapshotRequestError::InvalidHeader)?;
        let value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|_| AdminSnapshotRequestError::InvalidHeader)?;
        self.headers.insert(name, value);
        Ok(self)
    }
}

impl Default for AdminSnapshotRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminSnapshotDiagnostic {
    pub operation: AdminSnapshotOperation,
    pub status: u16,
    pub body: String,
    pub truncated: bool,
}

#[derive(Debug)]
pub enum AdminSnapshotRequestError {
    InvalidHeader,
    CredentialFailed { operation: AdminSnapshotOperation },
    RequestFailed { operation: AdminSnapshotOperation },
    UnexpectedStatus { diagnostic: AdminSnapshotDiagnostic },
    ResponseReadFailed { operation: AdminSnapshotOperation },
}

impl AdminSnapshotRequestError {
    pub fn diagnostic(&self) -> Option<&AdminSnapshotDiagnostic> {
        match self {
            Self::UnexpectedStatus { diagnostic } => Some(diagnostic),
            _ => None,
        }
    }
}

impl fmt::Display for AdminSnapshotRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => f.write_str("admin snapshot request header is invalid"),
            Self::CredentialFailed { operation } => write!(
                f,
                "admin snapshot {} credential could not be read or validated",
                operation.as_str()
            ),
            Self::RequestFailed { operation } => {
                write!(f, "admin snapshot {} request failed", operation.as_str())
            }
            Self::UnexpectedStatus { diagnostic } => write!(
                f,
                "admin snapshot {} returned status {}: {}{}",
                diagnostic.operation.as_str(),
                diagnostic.status,
                diagnostic.body,
                if diagnostic.truncated {
                    " [truncated]"
                } else {
                    ""
                }
            ),
            Self::ResponseReadFailed { operation } => write!(
                f,
                "admin snapshot {} response read failed",
                operation.as_str()
            ),
        }
    }
}

impl std::error::Error for AdminSnapshotRequestError {}

fn redacted_error(error: AdminSnapshotRequestError) -> AdminSnapshotTransportError {
    match error {
        AdminSnapshotRequestError::UnexpectedStatus { diagnostic } => {
            AdminSnapshotTransportError::UnexpectedStatus {
                operation: diagnostic.operation,
                status: reqwest::StatusCode::from_u16(diagnostic.status)
                    .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        AdminSnapshotRequestError::ResponseReadFailed { operation } => {
            AdminSnapshotTransportError::ResponseReadFailed { operation }
        }
        AdminSnapshotRequestError::InvalidHeader
        | AdminSnapshotRequestError::CredentialFailed { .. }
        | AdminSnapshotRequestError::RequestFailed { .. } => {
            AdminSnapshotTransportError::RequestFailed {
                operation: AdminSnapshotOperation::Backup,
            }
        }
    }
}

/// Strict, byte-preserving transport for the standard admin snapshot endpoints.
pub struct AdminSnapshotTransport {
    client: reqwest::Client,
    config: AdminSnapshotTransportConfig,
}

impl AdminSnapshotTransport {
    /// Construct a transport with production timeouts and no redirect policy.
    pub fn new() -> std::result::Result<Self, AdminSnapshotTransportError> {
        Self::with_config(AdminSnapshotTransportConfig::default())
    }

    pub fn with_config(
        config: AdminSnapshotTransportConfig,
    ) -> std::result::Result<Self, AdminSnapshotTransportError> {
        let client = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout())
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(|_| AdminSnapshotTransportError::ClientBuildFailed)?;
        Ok(Self { client, config })
    }

    #[cfg(test)]
    pub(super) fn for_tests(
        operation: std::time::Duration,
        backup_idle: std::time::Duration,
    ) -> Self {
        Self::with_config(
            AdminSnapshotTransportConfig::default()
                .with_operation_timeout(operation)
                .with_response_idle_timeout(backup_idle),
        )
        .expect("test client builds")
    }

    /// Fetch exact snapshot bytes with a product-supplied credential and
    /// metadata policy. Status diagnostics are bounded by the transport config.
    pub async fn fetch(
        &self,
        base_url: &str,
        policy: &AdminSnapshotRequest,
    ) -> std::result::Result<Vec<u8>, AdminSnapshotRequestError> {
        let operation = AdminSnapshotOperation::Backup;
        let result = tokio::time::timeout(self.config.operation_timeout(), async {
            let url = format!("{}/admin/backup", base_url.trim_end_matches('/'));
            let mut request = self.client.get(url).headers(policy.headers.clone());
            match &policy.credential {
                AdminCredential::None => {}
                AdminCredential::Static(token) => {
                    request = request.bearer_auth(token);
                }
                AdminCredential::Projected(source) => {
                    let token = source
                        .bearer_token()
                        .map_err(|_| AdminSnapshotRequestError::CredentialFailed { operation })?;
                    request = request.bearer_auth(token);
                }
            }
            let mut response = request
                .send()
                .await
                .map_err(|_| AdminSnapshotRequestError::RequestFailed { operation })?;
            if response.status() != reqwest::StatusCode::OK {
                let status = response.status().as_u16();
                let mut body = Vec::new();
                let limit = self.config.max_diagnostic_bytes();
                let mut truncated = false;
                loop {
                    let chunk =
                        tokio::time::timeout(self.config.response_idle_timeout(), response.chunk())
                            .await
                            .map_err(|_| AdminSnapshotRequestError::ResponseReadFailed {
                                operation,
                            })?
                            .map_err(|_| AdminSnapshotRequestError::ResponseReadFailed {
                                operation,
                            })?;
                    let Some(chunk) = chunk else { break };
                    let remaining = limit.saturating_sub(body.len());
                    if chunk.len() > remaining {
                        body.extend_from_slice(&chunk[..remaining]);
                        truncated = true;
                        break;
                    }
                    body.extend_from_slice(&chunk);
                    if body.len() == limit {
                        let more = tokio::time::timeout(
                            self.config.response_idle_timeout(),
                            response.chunk(),
                        )
                        .await
                        .map_err(|_| AdminSnapshotRequestError::ResponseReadFailed { operation })?
                        .map_err(|_| AdminSnapshotRequestError::ResponseReadFailed { operation })?;
                        truncated = more.is_some();
                        break;
                    }
                }
                return Err(AdminSnapshotRequestError::UnexpectedStatus {
                    diagnostic: AdminSnapshotDiagnostic {
                        operation,
                        status,
                        body: String::from_utf8_lossy(&body).into_owned(),
                        truncated,
                    },
                });
            }
            let mut payload = Vec::new();
            loop {
                let chunk =
                    tokio::time::timeout(self.config.response_idle_timeout(), response.chunk())
                        .await
                        .map_err(|_| AdminSnapshotRequestError::ResponseReadFailed { operation })?
                        .map_err(|_| AdminSnapshotRequestError::ResponseReadFailed { operation })?;
                let Some(chunk) = chunk else { break };
                payload.extend_from_slice(&chunk);
            }
            Ok(payload)
        })
        .await;
        result.map_err(|_| AdminSnapshotRequestError::RequestFailed { operation })?
    }

    /// Fetch the exact bytes returned by one `GET /admin/backup` request.
    pub async fn fetch_exact(
        &self,
        base_url: &str,
        bearer: Option<&str>,
    ) -> std::result::Result<Vec<u8>, AdminSnapshotTransportError> {
        let mut policy = AdminSnapshotRequest::new();
        if let Some(bearer) = bearer {
            policy = policy.with_static_bearer(bearer);
        }
        self.fetch(base_url, &policy).await.map_err(redacted_error)
    }

    /// Restore exact snapshot bytes with one `POST /admin/restore` request.
    pub async fn restore_exact(
        &self,
        base_url: &str,
        bearer: Option<&str>,
        snapshot: &[u8],
    ) -> std::result::Result<(), AdminSnapshotTransportError> {
        let operation = AdminSnapshotOperation::Restore;
        tokio::time::timeout(self.config.operation_timeout(), async {
            let url = format!("{}/admin/restore", base_url.trim_end_matches('/'));
            let mut request = self
                .client
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(snapshot.to_vec());
            if let Some(token) = bearer {
                request = request.bearer_auth(token);
            }
            let response = request
                .send()
                .await
                .map_err(|_| AdminSnapshotTransportError::RequestFailed { operation })?;
            if response.status() != reqwest::StatusCode::NO_CONTENT {
                return Err(AdminSnapshotTransportError::UnexpectedStatus {
                    operation,
                    status: response.status(),
                });
            }
            Ok(())
        })
        .await
        .map_err(|_| AdminSnapshotTransportError::RequestFailed { operation })?
    }
}
