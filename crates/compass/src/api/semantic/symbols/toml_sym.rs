//! TOML symbol extraction (line-based)
//!
//! Extracts symbols from TOML files:
//! - Sections `[name]` -> Module
//! - Key-value pairs `key = value` -> Variable
//! - Array tables `[[name]]` -> Module
