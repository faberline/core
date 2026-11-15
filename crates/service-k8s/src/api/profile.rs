//! The service profile contract and its pure validation, for apps (ADR 0002 D4).

pub use crate::domain::profile::{
    validate_profile, AlertRule, Capabilities, Partitioning, ProfileError, ReadScaling,
    ServiceProfile,
};
