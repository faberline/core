//! Typed ServiceAccount plan with metadata annotations (e.g. Workload Identity).

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{RenderCtx, WorkloadPlanError};

/// The annotation key for Google Cloud Workload Identity on Kubernetes ServiceAccounts.
pub const WORKLOAD_IDENTITY_ANNOTATION: &str = "iam.gke.io/gcp-service-account";

/// Validate that a GCP service account email matches `<name>@<project>.iam.gserviceaccount.com`.
pub fn is_valid_workload_identity(email: &str) -> bool {
    if email.is_empty() {
        return false;
    }
    let Some((name, domain)) = email.split_once('@') else {
        return false;
    };
    if name.is_empty() || name.contains('@') {
        return false;
    }
    let Some(project) = domain.strip_suffix(".iam.gserviceaccount.com") else {
        return false;
    };
    if project.is_empty() || project.contains('@') {
        return false;
    }
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceAccountIdentityPlan {
    pub name: String,
    pub component: String,
    pub annotations: BTreeMap<String, String>,
    pub automount_service_account_token: bool,
}

impl ServiceAccountIdentityPlan {
    pub fn new(name: impl Into<String>, component: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            annotations: BTreeMap::new(),
            automount_service_account_token: false,
        }
    }

    pub fn with_annotation(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.annotations.insert(key.into(), value.into());
        self
    }

    pub fn with_workload_identity(mut self, gcp_service_account_email: impl Into<String>) -> Self {
        let email = gcp_service_account_email.into();
        self.annotations
            .insert(WORKLOAD_IDENTITY_ANNOTATION.into(), email);
        self
    }

    pub fn try_with_workload_identity(
        mut self,
        gcp_service_account_email: impl Into<String>,
    ) -> Result<Self, WorkloadPlanError> {
        let email = gcp_service_account_email.into();
        if !is_valid_workload_identity(&email) {
            return Err(WorkloadPlanError::InvalidWorkloadIdentity(self.name, email));
        }
        self.annotations
            .insert(WORKLOAD_IDENTITY_ANNOTATION.into(), email);
        Ok(self)
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

    pub fn automount_service_account_token(&self) -> bool {
        self.automount_service_account_token
    }

    pub fn workload_identity(&self) -> Option<&str> {
        self.annotations
            .get(WORKLOAD_IDENTITY_ANNOTATION)
            .map(String::as_str)
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Result<Value, WorkloadPlanError> {
        for (key, value) in &self.annotations {
            if key == WORKLOAD_IDENTITY_ANNOTATION && !is_valid_workload_identity(value) {
                return Err(WorkloadPlanError::InvalidWorkloadIdentity(
                    self.name,
                    value.clone(),
                ));
            }
        }
        let mut metadata = cx.meta(&self.name, &self.component);
        if !self.annotations.is_empty() {
            metadata["annotations"] = json!(self.annotations);
        }
        Ok(json!({
            "apiVersion": "v1",
            "kind": "ServiceAccount",
            "metadata": metadata,
            "automountServiceAccountToken": self.automount_service_account_token,
        }))
    }
}
