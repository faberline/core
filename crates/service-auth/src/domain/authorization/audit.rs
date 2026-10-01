use serde::Serialize;

use super::role::Role;

/// Stable authorization outcomes exposed to audit/metrics adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationDecision {
    Allow,
    Deny,
}

/// Machine-stable decision reasons. None can carry credential bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationReason {
    Authorized,
    OpenMode,
    MissingBearer,
    UnknownBearer,
    InsufficientRole,
}

/// Machine-stable reload failure classes. Detailed parser/I/O errors are
/// returned to the caller, but the event surface does not echo input bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReloadFailure {
    Read,
    Parse,
    Invalid,
}

/// Redacted lifecycle/audit event delivered to a caller-selected backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AuthEvent {
    RegistryReload {
        applied: bool,
        revision: u64,
        entries: usize,
        failure: Option<ReloadFailure>,
    },
    AuthorizationDecision {
        decision: AuthorizationDecision,
        reason: AuthorizationReason,
        subject: Option<String>,
        resource: Option<String>,
        needed: Option<Role>,
    },
}

/// Backend-neutral hook for logs, metrics, or SIEM adapters.
pub trait AuthEventSink: Send + Sync {
    fn record(&self, event: &AuthEvent);
}

/// Sink used by tests/dev callers that do not want lifecycle telemetry.
#[derive(Debug, Default)]
pub struct NoopAuthEventSink;

impl AuthEventSink for NoopAuthEventSink {
    fn record(&self, _event: &AuthEvent) {}
}
