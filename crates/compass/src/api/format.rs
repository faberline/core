//! Formatter integration — unified interface for external formatters
//!
//! Wraps rustfmt, prettier, gofmt, black, terraform fmt, etc.

pub mod detect;

pub use crate::infrastructure::format::registry::{
    FormatResult, FormatterConfig, FormatterRegistry,
};
