use crate::domain::authorization::{AuthEvent, AuthEventSink, AuthorizationDecision};

/// Shared structured tracing adapter. Its fields are copied exclusively from
/// [`AuthEvent`], whose schema cannot represent a bearer credential.
#[derive(Debug, Default)]
pub struct TracingAuthEventSink;

impl AuthEventSink for TracingAuthEventSink {
    fn record(&self, event: &AuthEvent) {
        match event {
            AuthEvent::RegistryReload {
                applied,
                revision,
                entries,
                failure,
            } => {
                if *applied {
                    tracing::info!(
                        target: "service_auth.audit",
                        event = "credential_registry_reload",
                        applied,
                        revision,
                        entries,
                    );
                } else {
                    tracing::warn!(
                        target: "service_auth.audit",
                        event = "credential_registry_reload",
                        applied,
                        revision,
                        entries,
                        failure = ?failure,
                    );
                }
            }
            AuthEvent::AuthorizationDecision {
                decision,
                reason,
                subject,
                resource,
                needed,
            } => {
                let subject = subject.as_deref().unwrap_or("anonymous");
                let resource = resource.as_deref().unwrap_or("-");
                match decision {
                    AuthorizationDecision::Allow => tracing::debug!(
                        target: "service_auth.audit",
                        event = "authorization_decision",
                        decision = ?decision,
                        reason = ?reason,
                        subject,
                        resource,
                        needed = ?needed,
                    ),
                    AuthorizationDecision::Deny => tracing::warn!(
                        target: "service_auth.audit",
                        event = "authorization_decision",
                        decision = ?decision,
                        reason = ?reason,
                        subject,
                        resource,
                        needed = ?needed,
                    ),
                }
            }
        }
    }
}
