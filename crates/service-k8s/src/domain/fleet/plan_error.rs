//! Errors produced during Fleet planning.

use thiserror::Error;

use super::ChildKey;

/// Errors returned by [`plan_fleet`](super::plan_fleet).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FleetPlanError {
    #[error("duplicate desired target: {0}")]
    DuplicateTarget(ChildKey),
}
