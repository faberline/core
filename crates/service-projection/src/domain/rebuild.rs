use super::ProjectionCursor;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RebuildComparison {
    pub source_cursor: ProjectionCursor,
    pub rebuilt_cursor: ProjectionCursor,
    pub live_digest: String,
    pub rebuilt_digest: String,
    pub equal: bool,
}
