use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::digest::sha256;
use super::projection::ProjectionDescriptor;
use super::{ProjectionCursor, ProjectionEventId, ProjectionName, SourceGeneration};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ProjectionCheckpoint {
    #[schema(value_type = String)]
    pub projection: ProjectionName,
    pub schema_version: u32,
    #[schema(value_type = u64)]
    pub cursor: ProjectionCursor,
    #[serde(default)]
    #[schema(value_type = u64)]
    pub source_generation: SourceGeneration,
    #[schema(value_type = Option<String>)]
    pub event_id: Option<ProjectionEventId>,
    pub state_sha256: String,
    pub updated_at: String,
}

impl ProjectionCheckpoint {
    /// The checkpoint of a projection with no applied records, stamped with
    /// `now`.
    pub fn empty(descriptor: &ProjectionDescriptor, now: DateTime<Utc>) -> Self {
        Self {
            projection: descriptor.name().clone(),
            schema_version: descriptor.schema_version(),
            cursor: ProjectionCursor::default(),
            source_generation: SourceGeneration::default(),
            event_id: None,
            state_sha256: String::new(),
            updated_at: timestamp(now),
        }
    }
}

pub(crate) fn checkpoint(
    descriptor: &ProjectionDescriptor,
    cursor: ProjectionCursor,
    source_generation: SourceGeneration,
    event_id: Option<ProjectionEventId>,
    state: &[u8],
    now: DateTime<Utc>,
) -> ProjectionCheckpoint {
    ProjectionCheckpoint {
        projection: descriptor.name().clone(),
        schema_version: descriptor.schema_version(),
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
