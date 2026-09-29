//! The port the lifecycle reads and writes its one Secret through, and the
//! errors that port reports.

use std::collections::BTreeMap;

use serde_json::Value;

use super::status::redact;

/// A Secret as read out of the cluster, already base64-decoded.
#[derive(Clone, Debug, Default)]
pub struct StoredSecret {
    pub data: BTreeMap<String, Vec<u8>>,
    pub annotations: BTreeMap<String, String>,
}

/// Reading and writing the one Secret this lifecycle owns.
///
/// A trait rather than a `kube::Api` so the lifecycle is testable without a
/// cluster, and — the reason that matters more — so the set of operations is
/// finite and inspectable. There is no `delete`.
pub trait SecretStore: Send + Sync {
    fn read<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> futures::future::BoxFuture<'a, Result<Option<StoredSecret>, StoreError>>;

    /// Apply `object` with merge semantics. Trust-only projection widens
    /// `ca.crt` without deleting live leaf keys; the Kubernetes store
    /// materializes omitted lifecycle-owned fields before server-side apply.
    fn apply<'a>(
        &'a self,
        object: Value,
    ) -> futures::future::BoxFuture<'a, Result<(), StoreError>>;
}

/// Classification of errors arising from a [`SecretStore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreErrorKind {
    /// HTTP 403: Forbidden - configuration/RBAC fault, not retryable.
    Forbidden,
    /// HTTP 409: Conflict - SSA field conflict or resource version mismatch, retryable.
    Conflict,
    /// 5xx HTTP error or transport failure - transient unavailability, retryable.
    Unavailable,
    /// Malformed input object or invalid structure - configuration fault, not retryable.
    Malformed,
    /// Other API error with status code.
    Other(u16),
}

/// Why a Secret could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError {
    pub kind: StoreErrorKind,
    pub message: String,
}

impl StoreError {
    pub fn new(kind: StoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: redact(&message.into()),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StoreErrorKind::Forbidden, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StoreErrorKind::Conflict, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(StoreErrorKind::Unavailable, message)
    }

    pub fn malformed(message: impl Into<String>) -> Self {
        Self::new(StoreErrorKind::Malformed, message)
    }

    pub fn kind(&self) -> &StoreErrorKind {
        &self.kind
    }

    pub fn retryable(&self) -> bool {
        match self.kind {
            StoreErrorKind::Conflict | StoreErrorKind::Unavailable => true,
            StoreErrorKind::Forbidden | StoreErrorKind::Malformed => false,
            StoreErrorKind::Other(code) => code >= 500 || code == 429,
        }
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            StoreErrorKind::Forbidden => write!(f, "forbidden: {}", self.message),
            StoreErrorKind::Conflict => write!(f, "conflict: {}", self.message),
            StoreErrorKind::Unavailable => write!(f, "unavailable: {}", self.message),
            StoreErrorKind::Malformed => write!(f, "malformed object: {}", self.message),
            StoreErrorKind::Other(code) => write!(f, "api error ({code}): {}", self.message),
        }
    }
}

impl std::error::Error for StoreError {}
