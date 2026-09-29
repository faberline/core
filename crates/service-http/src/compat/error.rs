//! The shared HTTP error envelope every k8s-native service in the
//! ecosystem renders for its error responses. lumen established this shape
//! first (a `StorageError` → status/kind classification over a
//! `{"error", "message"}` body); this module is the one place it lives so
//! `keep`/`relay`/`loom` converge on the same JSON instead of hand-rolling a
//! coincidentally-similar one. `crates/service-auth`'s own rejection
//! rendering predates this module and is a later convergence — untouched
//! here.
//!
//! [`ErrorEnvelope`] is the wire shape; [`ApiErr`] pairs a `StatusCode` with
//! a short machine-stable `kind` and a human `message`, and renders as
//! [`ErrorEnvelope`] JSON via `IntoResponse`. A service builds one per
//! domain-error classification arm ([`ApiErr::new`]) — this crate only owns
//! the generic envelope + builder, never the domain classification, which
//! stays in the service's own `From<DomainError>` impl.

pub use crate::interfaces::{
    retry_delay_from_detailed_error, ApiErr, DetailedErrorEnvelope, ErrorEnvelope,
    ProjectionMetadata,
};
