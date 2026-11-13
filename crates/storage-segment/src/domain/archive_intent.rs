use serde::{Deserialize, Serialize};

use super::{catalog::hex_sha256, ArchiveObject, Result, SegmentError};

/// The reserved key prefix under which archive intents are stored.
pub const ARCHIVE_INTENT_PREFIX: &str = "_archive/intent/";

/// The persisted format version of an `ArchiveIntent`.
pub const ARCHIVE_INTENT_FORMAT_VERSION: u32 = 1;

/// The record that says "this immutable segment object will be uploaded".
/// Field order fixes the persisted JSON bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArchiveIntent {
    pub format_version: u32,
    pub segment_key: String,
    pub size: u64,
    pub sha256: String,
    pub content_type: String,
}

impl ArchiveIntent {
    pub fn new(
        segment_key: impl Into<String>,
        size: u64,
        sha256: impl Into<String>,
        content_type: impl Into<String>,
    ) -> Self {
        Self {
            format_version: ARCHIVE_INTENT_FORMAT_VERSION,
            segment_key: segment_key.into(),
            size,
            sha256: sha256.into(),
            content_type: content_type.into(),
        }
    }

    /// The intent of an archive object: its key, size, sha256 and content type.
    pub fn for_object(object: &ArchiveObject) -> Self {
        Self::new(
            object.key.clone(),
            object.bytes.len() as u64,
            hex_sha256(&object.bytes),
            object.content_type.clone(),
        )
    }

    /// The object key of this intent. The hash keeps the key free of `/` and
    /// of any character the store refuses.
    pub fn object_key(&self) -> String {
        format!(
            "{ARCHIVE_INTENT_PREFIX}{}.json",
            hex_sha256(self.segment_key.as_bytes())
        )
    }

    /// The versioned JSON bytes of this intent.
    pub fn encode(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|error| SegmentError::Serialization {
            message: error.to_string(),
        })
    }

    /// Decode an intent. A wrong version is `UnsupportedArchiveIntentVersion`;
    /// any other damage is `CorruptArchiveIntent`.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| SegmentError::CorruptArchiveIntent {
                message: error.to_string(),
            })?;
        let Some(version) = value.get("format_version").and_then(|v| v.as_u64()) else {
            return Err(SegmentError::CorruptArchiveIntent {
                message: "missing format_version".to_string(),
            });
        };
        if version != u64::from(ARCHIVE_INTENT_FORMAT_VERSION) {
            return Err(SegmentError::UnsupportedArchiveIntentVersion { version });
        }
        serde_json::from_value(value).map_err(|error| SegmentError::CorruptArchiveIntent {
            message: error.to_string(),
        })
    }
}

#[cfg(test)]
mod tests;
