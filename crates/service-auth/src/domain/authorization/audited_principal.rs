use std::fmt;
use std::sync::Arc;

use super::audit::{AuthEvent, AuthEventSink, AuthorizationDecision, AuthorizationReason};
use super::principal::{RoleMapDenied, RoleMapPrincipal};
use super::role::Role;

/// A principal carrying the shared audit sink used by its verifier.
#[derive(Clone)]
pub struct AuditedRoleMapPrincipal {
    pub(crate) inner: RoleMapPrincipal,
    pub(crate) sink: Arc<dyn AuthEventSink>,
}

impl fmt::Debug for AuditedRoleMapPrincipal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditedRoleMapPrincipal")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl AuditedRoleMapPrincipal {
    pub fn ensure(&self, resource: &str, needed: Role) -> std::result::Result<(), RoleMapDenied> {
        match self.inner.ensure(resource, needed) {
            Ok(()) => {
                self.sink.record(&AuthEvent::AuthorizationDecision {
                    decision: AuthorizationDecision::Allow,
                    reason: if self.subject().is_some() {
                        AuthorizationReason::Authorized
                    } else {
                        AuthorizationReason::OpenMode
                    },
                    subject: self.subject().map(str::to_owned),
                    resource: Some(resource.to_owned()),
                    needed: Some(needed),
                });
                Ok(())
            }
            Err(denied) => {
                self.sink.record(&AuthEvent::AuthorizationDecision {
                    decision: AuthorizationDecision::Deny,
                    reason: AuthorizationReason::InsufficientRole,
                    subject: Some(denied.subject.clone()),
                    resource: Some(denied.resource.clone()),
                    needed: Some(denied.needed),
                });
                Err(denied)
            }
        }
    }

    pub fn subject(&self) -> Option<&str> {
        self.inner.subject()
    }

    pub fn into_inner(self) -> RoleMapPrincipal {
        self.inner
    }
}
