//! Role-map authorization: roles and token claims, the two-namespace
//! credential registry, the resolved principal, and the audit events it emits.

mod audit;
mod audited_principal;
mod principal;
mod registry;
mod registry_error;
mod role;

pub use audit::{
    AuthEvent, AuthEventSink, AuthorizationDecision, AuthorizationReason, NoopAuthEventSink,
    ReloadFailure,
};
pub use audited_principal::AuditedRoleMapPrincipal;
pub use principal::{RoleMapDenied, RoleMapPrincipal};
pub use registry::Registry;
pub use registry_error::RegistryError;
pub use role::{Role, TokenClaims};
