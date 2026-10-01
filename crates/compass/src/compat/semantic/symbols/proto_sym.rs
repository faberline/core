//! Protocol Buffer (proto3) symbol extraction (line-based)
//!
//! Extracts symbols from proto files:
//! - message -> Class
//! - field -> Resource (field)
//! - service -> Interface
//! - rpc -> Function
//! - enum -> Enum
