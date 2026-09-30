use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A document to index: an external id, a version and named text fields.
/// `new` and `with_field` build it; `Deserialize` fills the fields directly.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextDocument {
    external_id: String,
    version: u64,
    fields: BTreeMap<String, String>,
}

impl TextDocument {
    pub fn new(external_id: impl Into<String>, version: u64) -> Self {
        Self {
            external_id: external_id.into(),
            version,
            fields: BTreeMap::new(),
        }
    }

    pub fn with_field(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    pub fn external_id(&self) -> &str {
        &self.external_id
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn fields(&self) -> &BTreeMap<String, String> {
        &self.fields
    }
}

#[cfg(test)]
mod tests;
