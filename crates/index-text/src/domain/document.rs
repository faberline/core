use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{DocumentId, DocumentVersion};

/// A document to index: an external id, a version and named text fields.
/// `new` and `with_field` build it; `Deserialize` fills the fields directly.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextDocument {
    external_id: DocumentId,
    version: DocumentVersion,
    fields: BTreeMap<String, String>,
}

impl TextDocument {
    /// Build a document with a key and a write version.
    ///
    /// ```compile_fail
    /// use index_text::{DocumentId, DocumentVersion, TextDocument};
    /// TextDocument::new(DocumentVersion::new(1), DocumentId::new("doc"));
    /// ```
    pub fn new(external_id: DocumentId, version: DocumentVersion) -> Self {
        Self {
            external_id,
            version,
            fields: BTreeMap::new(),
        }
    }

    pub fn with_field(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    pub fn external_id(&self) -> &DocumentId {
        &self.external_id
    }

    pub fn version(&self) -> DocumentVersion {
        self.version
    }

    pub fn fields(&self) -> &BTreeMap<String, String> {
        &self.fields
    }
}

#[cfg(test)]
mod tests;
