use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ObjectVersion(String);

impl ObjectVersion {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Metadata of one stored object. `Deserialize` fills the fields directly.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObjectMeta {
    key: String,
    size: u64,
    content_type: String,
    version: ObjectVersion,
    etag: Option<String>,
    updated: Option<String>,
}

impl ObjectMeta {
    /// Metadata with no etag and no update time; add them with
    /// [`with_etag`](Self::with_etag) and [`with_updated`](Self::with_updated).
    pub fn new(
        key: impl Into<String>,
        size: u64,
        content_type: impl Into<String>,
        version: ObjectVersion,
    ) -> Self {
        Self {
            key: key.into(),
            size,
            content_type: content_type.into(),
            version,
            etag: None,
            updated: None,
        }
    }

    /// Set the backend's entity tag.
    pub fn with_etag(mut self, etag: Option<String>) -> Self {
        self.etag = etag;
        self
    }

    /// Set the backend's update time, as the backend reports it.
    pub fn with_updated(mut self, updated: Option<String>) -> Self {
        self.updated = updated;
        self
    }

    /// Key relative to the store's configured prefix.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// The version a conditional put compares against.
    pub fn version(&self) -> &ObjectVersion {
        &self.version
    }

    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    pub fn updated(&self) -> Option<&str> {
        self.updated.as_deref()
    }
}

/// A stored object: its metadata and bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Object {
    meta: ObjectMeta,
    bytes: Vec<u8>,
}

impl Object {
    pub fn new(meta: ObjectMeta, bytes: Vec<u8>) -> Self {
        Self { meta, bytes }
    }

    pub fn meta(&self) -> &ObjectMeta {
        &self.meta
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Move out the metadata and the bytes.
    pub fn into_parts(self) -> (ObjectMeta, Vec<u8>) {
        (self.meta, self.bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PutCondition {
    Any,
    IfAbsent,
    IfVersion(ObjectVersion),
}

#[cfg(test)]
mod tests;
