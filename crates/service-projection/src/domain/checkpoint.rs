use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::digest::sha256;
use super::projection::ProjectionDescriptor;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ProjectionCheckpoint {
    pub projection: String,
    pub schema_version: u32,
    pub cursor: u64,
    #[serde(default)]
    pub source_generation: u64,
    pub event_id: Option<String>,
    pub state_sha256: String,
    pub updated_at: String,
}

impl ProjectionCheckpoint {
    /// The checkpoint of a projection with no applied records, stamped with
    /// `now`.
    pub fn empty(descriptor: &ProjectionDescriptor, now: DateTime<Utc>) -> Self {
        Self {
            projection: descriptor.name.clone(),
            schema_version: descriptor.schema_version,
            cursor: 0,
            source_generation: 0,
            event_id: None,
            state_sha256: String::new(),
            updated_at: timestamp(now),
        }
    }
}

pub(crate) fn checkpoint(
    descriptor: &ProjectionDescriptor,
    cursor: u64,
    source_generation: u64,
    event_id: Option<String>,
    state: &[u8],
    now: DateTime<Utc>,
) -> ProjectionCheckpoint {
    ProjectionCheckpoint {
        projection: descriptor.name.clone(),
        schema_version: descriptor.schema_version,
        cursor,
        source_generation,
        event_id,
        state_sha256: sha256(state),
        updated_at: timestamp(now),
    }
}

/// `updated_at` text: RFC 3339 in UTC with milliseconds and a `Z` suffix.
fn timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod timestamp_tests;
