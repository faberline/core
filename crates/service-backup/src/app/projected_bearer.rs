//! `AdminSnapshotRequest::with_projected_bearer`: a projected ServiceAccount
//! token file as the transport's bearer token source.

use std::path::Path;
use std::sync::Arc;

use service_auth::k8s::ProjectedTokenFile;

use crate::domain::{BearerTokenError, BearerTokenSource};
use crate::infrastructure::AdminSnapshotRequest;

/// A projected token file, read and checked on every call.
struct ProjectedBearerToken(ProjectedTokenFile);

impl BearerTokenSource for ProjectedBearerToken {
    fn bearer_token(&self) -> Result<String, BearerTokenError> {
        let token = self.0.read().map_err(BearerTokenError::other)?;
        Ok(token.expose().to_owned())
    }
}

impl AdminSnapshotRequest {
    /// Store only the file descriptor policy. The file is opened and checked
    /// immediately before every request so kubelet token rotation is observed.
    pub fn with_projected_bearer(
        self,
        path: impl AsRef<Path>,
        audience: impl Into<String>,
    ) -> Self {
        self.with_bearer_source(Arc::new(ProjectedBearerToken(ProjectedTokenFile::new(
            path.as_ref(),
            audience,
        ))))
    }
}
