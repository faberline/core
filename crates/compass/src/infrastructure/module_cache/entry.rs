use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::domain::module_cache::content_hash::ContentHash;
use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::TypeError;

/// Cached module entry
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Module name
    pub module_name: String,
    /// File path
    pub path: PathBuf,
    /// Content hash for change detection
    pub content_hash: ContentHash,
    /// Modification time (for quick change detection)
    pub mtime: Option<SystemTime>,
    /// Cached module info
    pub info: ModuleInfo,
    /// Cached type errors
    pub errors: Vec<TypeError>,
    /// Modules this module depends on
    pub dependencies: HashSet<String>,
    /// Whether cross-file propagated types are still valid (R3, R8).
    pub propagation_valid: bool,
}

impl CacheEntry {
    pub fn new(
        module_name: String,
        path: PathBuf,
        content_hash: ContentHash,
        info: ModuleInfo,
    ) -> Self {
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        Self {
            module_name,
            path,
            content_hash,
            mtime,
            info,
            errors: Vec::new(),
            dependencies: HashSet::new(),
            propagation_valid: false,
        }
    }

    /// Check if file has changed based on mtime (fast check)
    pub fn mtime_changed(&self) -> bool {
        let current_mtime = fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok());

        match (self.mtime, current_mtime) {
            (Some(cached), Some(current)) => cached != current,
            _ => true, // Assume changed if we can't determine
        }
    }

    /// Check if file content has changed (slower but accurate)
    pub fn content_changed(&self) -> bool {
        match ContentHash::from_file(&self.path) {
            Some(current) => current != self.content_hash,
            None => true, // File removed or unreadable
        }
    }

    /// Check if this entry needs reanalysis
    pub fn needs_reanalysis(&self) -> bool {
        // Quick mtime check first
        if !self.mtime_changed() {
            return false;
        }
        // Fall back to content hash check
        self.content_changed()
    }
}
