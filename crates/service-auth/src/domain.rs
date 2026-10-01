//! Pure authorization and identity concepts: roles, claims and the
//! credential registry, the resolved principal and its audit events, the
//! Google verifier's configuration and errors, and the Kubernetes principal,
//! cache and token values.

pub(crate) mod authorization;
pub(crate) mod google;
pub(crate) mod k8s;
