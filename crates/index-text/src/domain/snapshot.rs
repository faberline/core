use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::document::TextDocument;
use super::error::{IndexError, Result};
use super::schema::TextSchema;
use super::{DocumentId, DocumentVersion};

pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextIndexSnapshot {
    pub format_version: u32,
    pub schema: TextSchema,
    pub documents: Vec<TextDocument>,
    /// Highest observed delete version for each absent document. Older version
    /// 1 snapshots omit this field and decode with an empty tombstone table.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tombstones: BTreeMap<DocumentId, DocumentVersion>,
}

impl TextIndexSnapshot {
    pub fn encode(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|error| IndexError::CorruptSnapshot {
            message: error.to_string(),
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|error| IndexError::CorruptSnapshot {
            message: error.to_string(),
        })
    }
}

#[cfg(test)]
mod tests;
