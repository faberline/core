//! Weighted quota admission with an RAII concurrency lease.

pub use crate::application::{
    ConcurrencyLease, WeightedAdmission, WeightedAdmissionConfig, WeightedAdmissionConfigError,
    WeightedAdmissionError,
};
