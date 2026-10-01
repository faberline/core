use serde::{Deserialize, Serialize};
use storage_object::ObjectVersion;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveObject {
    pub key: String,
    pub bytes: Vec<u8>,
    pub content_type: String,
}

impl ArchiveObject {
    pub fn new(key: impl Into<String>, bytes: Vec<u8>, content_type: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            bytes,
            content_type: content_type.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArchivedObject {
    pub key: String,
    pub size: u64,
    pub content_type: String,
    pub sha256: String,
    pub version: ObjectVersion,
}

/// Receipt returned only after the final manifest write succeeds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArchiveCommit {
    pub objects: Vec<ArchivedObject>,
    pub manifest: ArchivedObject,
}
