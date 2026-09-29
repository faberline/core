//! Bounded in-process request admission for HTTP services.
//!
//! Applications select endpoint classes and opaque request keys. This module
//! hashes a key before it reaches retained state, applies the configured token
//! bucket, and emits only class/outcome metadata to observers.

pub use crate::application::{
    AdmissionConfig, AdmissionConfigError, AdmissionController, AdmissionDecision, AdmissionEvent,
    AdmissionInput, AdmissionObserver, AdmissionOutcome, AdmissionPolicy, AdmissionPolicyError,
    NoopAdmissionObserver,
};
pub use crate::interfaces::{admission_middleware, AdmissionMiddleware};
