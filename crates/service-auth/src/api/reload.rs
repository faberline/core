//! Atomic credential-registry reload and redacted authorization audit.
//!
//! The verifier stores one validated role-map snapshot behind a short-lived
//! `RwLock`. Reload parsing and semantic validation happen before the write
//! lock is acquired, so a failed replacement cannot disturb the last-known-
//! good registry. Audit events intentionally have no credential field: an
//! unknown bearer is reported only as an authentication denial.

pub use crate::application::role_map::{
    spawn_registry_file_watcher, spawn_registry_file_watcher_with_interval,
    spawn_registry_files_watcher_with_interval, ReloadableRoleMapVerifier,
    DEFAULT_REGISTRY_FILE_WATCH_INTERVAL,
};
pub use crate::domain::authorization::{
    AuditedRoleMapPrincipal, AuthEvent, AuthEventSink, AuthorizationDecision, AuthorizationReason,
    NoopAuthEventSink, ReloadFailure,
};
pub use crate::infrastructure::TracingAuthEventSink;
