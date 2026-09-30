//! GraphQL symbol extraction (line-based)
//!
//! Extracts symbols from GraphQL schema/query files:
//! - type -> Class
//! - field -> Resource (field)
//! - query/mutation/subscription -> Function
//! - fragment -> Variable
//! - enum -> Enum
