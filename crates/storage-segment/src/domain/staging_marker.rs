use serde::{Deserialize, Serialize};

use super::{hex_sha256, Result, SegmentError};

/// The reserved key prefix under which staging markers are stored.
pub const STAGING_MARKER_PREFIX: &str = "_archive/stage/";

/// The persisted format version of a `StagingMarker`.
pub const STAGING_MARKER_FORMAT_VERSION: u32 = 1;

/// Where one archive object is in its staging life cycle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StagingState {
    /// The upload is about to start.
    Intent,
    /// The object is uploaded.
    Uploaded,
    /// The catalog commit names the object.
    Committed,
    /// The object waits for garbage collection.
    GcPending,
}

impl StagingState {
    /// Whether a marker may move from this state to `next`.
    pub fn can_move_to(self, next: StagingState) -> bool {
        matches!(
            (self, next),
            (StagingState::Intent, StagingState::Uploaded)
                | (StagingState::Uploaded, StagingState::Committed)
                | (StagingState::Intent, StagingState::GcPending)
                | (StagingState::Uploaded, StagingState::GcPending)
        )
    }
}

/// The versioned record of one segment's staging state. Field order fixes
/// the persisted JSON bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StagingMarker {
    pub format_version: u32,
    pub segment_key: String,
    pub state: StagingState,
    /// Caller-supplied seconds when the marker became `Committed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_at: Option<u64>,
    /// Caller-supplied seconds when the marker became `GcPending`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gc_pending_at: Option<u64>,
}

/// Segment keys that one GC pass deleted, in marker-key order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GcReport {
    pub deleted: Vec<String>,
}

impl StagingMarker {
    /// A marker for `segment_key` in the `Intent` state.
    pub fn new(segment_key: impl Into<String>) -> Self {
        Self {
            format_version: STAGING_MARKER_FORMAT_VERSION,
            segment_key: segment_key.into(),
            state: StagingState::Intent,
            committed_at: None,
            gc_pending_at: None,
        }
    }

    /// A copy of this marker in `next` that also records `at` (caller seconds)
    /// when `next` is `Committed` or `GcPending`.
    pub fn move_to_at(&self, next: StagingState, at: u64) -> Result<StagingMarker> {
        let mut moved = self.move_to(next)?;
        match next {
            StagingState::Committed => moved.committed_at = Some(at),
            StagingState::GcPending => moved.gc_pending_at = Some(at),
            StagingState::Intent | StagingState::Uploaded => {}
        }
        Ok(moved)
    }

    /// The object key of this marker.
    pub fn object_key(&self) -> String {
        staging_object_key(&self.segment_key)
    }

    /// A copy of this marker in `next`, when the transition is legal.
    pub fn move_to(&self, next: StagingState) -> Result<StagingMarker> {
        if !self.state.can_move_to(next) {
            return Err(SegmentError::IllegalStagingTransition {
                from: self.state,
                to: next,
            });
        }
        Ok(Self {
            state: next,
            ..self.clone()
        })
    }

    /// The versioned JSON bytes of this marker.
    pub fn encode(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|error| SegmentError::Serialization {
            message: error.to_string(),
        })
    }

    /// Decode a marker. A wrong version is `UnsupportedStagingMarkerVersion`;
    /// any other damage is `CorruptStagingMarker`.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| SegmentError::CorruptStagingMarker {
                message: error.to_string(),
            })?;
        let Some(version) = value.get("format_version").and_then(|v| v.as_u64()) else {
            return Err(SegmentError::CorruptStagingMarker {
                message: "missing format_version".to_string(),
            });
        };
        if version != u64::from(STAGING_MARKER_FORMAT_VERSION) {
            return Err(SegmentError::UnsupportedStagingMarkerVersion { version });
        }
        serde_json::from_value(value).map_err(|error| SegmentError::CorruptStagingMarker {
            message: error.to_string(),
        })
    }
}

/// The object key of a segment's marker. The hash keeps the key free of `/`
/// and of any character the store refuses.
pub(crate) fn staging_object_key(segment_key: &str) -> String {
    format!(
        "{STAGING_MARKER_PREFIX}{}.json",
        hex_sha256(segment_key.as_bytes())
    )
}

#[cfg(test)]
mod tests;
