//! TOML lint checker (line-based)
//!
//! Rules:
//! - TM001: Syntax error (toml parse failure)
//! - TM002: Duplicate table header
//! - TM003: Empty table (header with no key-value pairs)
//! - TM004: Deprecated Cargo.toml keys
//! - TM005: Long string values (> 200 chars, suggest multi-line)

pub use crate::domain::lint::toml_checker::TomlChecker;
