//! Durable activation for immutable directory generations.
//!
//! The caller owns the bytes and their domain validation. This module owns the
//! filesystem transaction that makes one complete generation current. The
//! rename of `CURRENT.tmp` over `CURRENT` is the only activation commit point.

mod commit;
mod commit_error;
mod current;
mod durability;
mod failure_injection;
mod inherited;
mod initialize;
mod name;
mod staging;
mod store;
mod tree;

pub use commit_error::{CommitError, CommitFailureClass};
pub use current::{
    CurrentReadError, CurrentReadErrorKind, CurrentTarget, CURRENT_FILE_NAME,
    CURRENT_TEMP_FILE_NAME, EMPTY_CURRENT_BYTES,
};
pub use failure_injection::{CommitStep, FailureInjector, FailurePoint, NoFailures};
pub use inherited::CurrentGenerationStaging;
pub use name::{GenerationName, GenerationNameError, GenerationNameErrorKind};
pub use staging::StagedGeneration;
pub use store::GenerationStore;

#[cfg(test)]
mod tests;
