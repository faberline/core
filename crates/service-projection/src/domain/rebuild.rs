use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RebuildComparison {
    pub source_cursor: u64,
    pub rebuilt_cursor: u64,
    pub live_digest: String,
    pub rebuilt_digest: String,
    pub equal: bool,
}
