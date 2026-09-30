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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObjectMeta {
    /// Key relative to the store's configured prefix.
    pub key: String,
    pub size: u64,
    pub content_type: String,
    pub version: ObjectVersion,
    pub etag: Option<String>,
    pub updated: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Object {
    pub meta: ObjectMeta,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PutCondition {
    Any,
    IfAbsent,
    IfVersion(ObjectVersion),
}

#[cfg(test)]
mod tests;
