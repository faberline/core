//! SQL lint checker (line-based)
//!
//! Rules: SQ001-SQ005, PG001, MY001
//! Also provides `detect_sql_injection` for Python/JS/Go code.

pub use crate::domain::lint::sql::{detect_sql_injection, SqlChecker};
