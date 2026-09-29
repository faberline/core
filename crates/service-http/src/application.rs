//! Use cases: token-bucket and weighted request admission, and the typed
//! admission configuration.

mod admission;
mod admission_config;
mod weighted_admission;

pub use admission::{
    AdmissionController, AdmissionDecision, AdmissionEvent, AdmissionInput, AdmissionObserver,
    AdmissionOutcome, AdmissionPolicy, AdmissionPolicyError, NoopAdmissionObserver,
};
pub use admission_config::{AdmissionConfig, AdmissionConfigError};
pub use weighted_admission::{
    ConcurrencyLease, WeightedAdmission, WeightedAdmissionConfig, WeightedAdmissionConfigError,
    WeightedAdmissionError,
};
