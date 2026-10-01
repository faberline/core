//! Persistent AST index cache backed by bincode.
//!
//! Each source file gets a `{path_hash}.idx` file in the cache directory.
//! A `manifest.bin` tracks path→hash mappings for fast staleness checks.

pub use crate::infrastructure::analysis_cache::disk_cache::{DiskCache, PersistedEntry};
