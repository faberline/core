use std::fs;
use std::path::Path;

use crate::domain::module_cache::content_hash::ContentHash;

impl ContentHash {
    /// Compute hash from file path
    pub fn from_file(path: &Path) -> Option<Self> {
        fs::read_to_string(path)
            .ok()
            .map(|c| Self::from_content(&c))
    }
}
