use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextDocument {
    pub external_id: String,
    pub version: u64,
    pub fields: BTreeMap<String, String>,
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
}
