//! SQL symbol extraction (line-based)
//!
//! Extracts symbols from SQL files:
//! - CREATE TABLE -> Class
//! - Column definitions -> Field
//! - CREATE FUNCTION/PROCEDURE -> Function
//! - CREATE INDEX -> Variable
