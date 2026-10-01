use serde::{Deserialize, Serialize};

use super::key::validate_catalog_key;
use crate::domain::Result;

/// One catalog entry: a key and its opaque value bytes. `Deserialize` fills
/// the fields directly; the catalog checks keys again when it builds or
/// reads pages.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogEntry {
    key: String,
    value: Vec<u8>,
}

impl CatalogEntry {
    /// An entry, or `InvalidCatalogKey` when the key is empty, longer than
    /// 1024 bytes or contains a NUL.
    pub fn try_new(key: impl Into<String>, value: Vec<u8>) -> Result<Self> {
        let key = key.into();
        validate_catalog_key(&key)?;
        Ok(Self { key, value })
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Move out the key and the value.
    pub fn into_parts(self) -> (String, Vec<u8>) {
        (self.key, self.value)
    }
}

#[cfg(test)]
mod tests;
