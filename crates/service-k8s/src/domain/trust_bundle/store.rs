//! ConfigMap store abstraction and errors.

use futures::future::BoxFuture;
use serde_json::Value;
use std::collections::BTreeMap;

/// A ConfigMap as read out of the cluster or memory store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoredConfigMap {
    pub data: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
}

/// Classification of errors arising from a [`ConfigMapStore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreErrorKind {
    /// HTTP 403: Forbidden - RBAC or authorization fault.
    Forbidden,
    /// HTTP 409: Conflict - resource version or SSA conflict.
    Conflict,
    /// 5xx HTTP error or transport failure - transient unavailability.
    Unavailable,
    /// Malformed input object or invalid structure.
    Malformed,
    /// Resource not found.
    NotFound,
    /// Other API error with status code.
    Other(u16),
}

/// Why a ConfigMap could not be read, written, or deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError {
    pub kind: StoreErrorKind,
    pub message: String,
}

impl StoreError {
    pub fn new(kind: StoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
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

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StoreErrorKind::NotFound, message)
    }

    pub fn retryable(&self) -> bool {
        match self.kind {
            StoreErrorKind::Conflict | StoreErrorKind::Unavailable => true,
            StoreErrorKind::Forbidden | StoreErrorKind::Malformed | StoreErrorKind::NotFound => {
                false
            }
            StoreErrorKind::Other(code) => code >= 500 || code == 429,
        }
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for StoreError {}

/// Reading, writing, and deleting ConfigMaps in declared client namespaces.
pub trait ConfigMapStore: Send + Sync {
    fn read<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<StoredConfigMap>, StoreError>>;

    fn apply<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
        object: Value,
    ) -> BoxFuture<'a, Result<(), StoreError>>;

    fn delete<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), StoreError>>;
}
