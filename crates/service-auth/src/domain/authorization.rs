//! Role-map authorization: roles and token claims, the two-namespace
//! credential registry, the resolved principal, and the audit events it emits.

mod audit;
mod audited_principal;
mod principal;
mod registry;
mod role;

pub use audit::{
    AuthEvent, AuthEventSink, AuthorizationDecision, AuthorizationReason, NoopAuthEventSink,
    ReloadFailure,
};
pub use audited_principal::AuditedRoleMapPrincipal;
pub use principal::{RoleMapDenied, RoleMapPrincipal};
pub use registry::Registry;
pub use role::{Role, TokenClaims};
