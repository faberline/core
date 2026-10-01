//! Watch mode for automatic re-analysis
//!
//! Provides file system watching with debouncing for incremental analysis.

pub use crate::infrastructure::watch::file_watcher::{FileWatcher, WatchConfig, WatchEvent};
