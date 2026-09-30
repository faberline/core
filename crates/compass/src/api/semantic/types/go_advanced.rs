//! Advanced Go type analysis
//!
//! Provides higher-level analysis built on top of GoTypeInference:
//! - Interface satisfaction checking
//! - Cross-type method set comparison
//! - Type assertion validation

pub use crate::domain::semantic::types::go_advanced::{
    check_interface_satisfaction, validate_type_assertions, SatisfactionResult,
};
