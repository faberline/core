use std::hash::{Hash, Hasher};

/// A content hash for change detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ContentHash(pub(crate) u64);

impl ContentHash {
    /// Compute hash from file content
    pub fn from_content(content: &str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        Self(hasher.finish())
    }
}
