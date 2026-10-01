use serde::{Deserialize, Serialize};

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
    pub version: ArchivedObjectVersion,
}

/// The object store's version of an archived object, such as a GCS
/// generation or a local content hash. It serializes as a bare string.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ArchivedObjectVersion(String);

impl ArchivedObjectVersion {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Receipt returned only after the final manifest write succeeds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArchiveCommit {
    pub objects: Vec<ArchivedObject>,
    pub manifest: ArchivedObject,
}
