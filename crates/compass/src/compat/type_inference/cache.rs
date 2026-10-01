//! File-level caching for incremental analysis
//!
//! This module provides:
//! - Content hashing for change detection
//! - Cached module info storage
//! - Dependency-aware cache invalidation

pub use crate::domain::module_cache::content_hash::ContentHash;
pub use crate::infrastructure::module_cache::analysis_cache::AnalysisCache;
pub use crate::infrastructure::module_cache::entry::CacheEntry;
