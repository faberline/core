//! A ServiceAccount that can carry annotations, for Workload Identity.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use thiserror::Error;

use super::RenderCtx;

const WORKLOAD_IDENTITY_ANNOTATION: &str = "iam.gke.io/gcp-service-account";
const GCP_SERVICE_ACCOUNT_SUFFIX: &str = ".iam.gserviceaccount.com";

/// A Workload Identity email that is not `<name>@<project>.iam.gserviceaccount.com`.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("Workload Identity email {0:?} is not <name>@<project>.iam.gserviceaccount.com")]
pub struct InvalidWorkloadIdentity(pub String);

/// A ServiceAccount with metadata annotations. It never mounts an API token:
/// `automountServiceAccountToken` is always `false`, and a pod that needs a
/// credential mounts a projected token of its own.
///
/// With no annotation set it renders the same object as
/// [`super::ServiceAccountPlan`] with `automount_service_account_token` false.
#[derive(Clone, Debug)]
pub struct ServiceAccountIdentityPlan {
    name: String,
    component: String,
    annotations: BTreeMap<String, String>,
}

impl ServiceAccountIdentityPlan {
    pub fn new(name: impl Into<String>, component: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            annotations: BTreeMap::new(),
        }
    }

    /// Sets one annotation. The Workload Identity key is checked here, so no
    /// path can render an invalid binding.
    pub fn with_annotation(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, InvalidWorkloadIdentity> {
        let key = key.into();
        let value = value.into();
        if key == WORKLOAD_IDENTITY_ANNOTATION && !is_gcp_service_account_email(&value) {
            return Err(InvalidWorkloadIdentity(value));
        }
        self.annotations.insert(key, value);
        Ok(self)
    }

    /// Binds the ServiceAccount to a Google service account through
    /// `iam.gke.io/gcp-service-account`. An empty or malformed email is an error.
    pub fn with_workload_identity(
        self,
        gcp_service_account_email: impl Into<String>,
    ) -> Result<Self, InvalidWorkloadIdentity> {
        self.with_annotation(WORKLOAD_IDENTITY_ANNOTATION, gcp_service_account_email)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn component(&self) -> &str {
        &self.component
    }

    pub fn annotations(&self) -> &BTreeMap<String, String> {
        &self.annotations
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let mut metadata = cx.meta(&self.name, &self.component);
        if !self.annotations.is_empty() {
            metadata["annotations"] = json!(self.annotations);
        }
        json!({
            "apiVersion": "v1",
            "kind": "ServiceAccount",
            "metadata": metadata,
            "automountServiceAccountToken": false,
        })
    }
}

/// `<name>@<project>.iam.gserviceaccount.com`, with a non-empty name and project.
fn is_gcp_service_account_email(email: &str) -> bool {
    let Some(local) = email.strip_suffix(GCP_SERVICE_ACCOUNT_SUFFIX) else {
        return false;
    };
    let Some((name, project)) = local.split_once('@') else {
        return false;
    };
    let name_ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let project_ok = !project.is_empty()
        && project
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':'));
    name_ok && project_ok
}
