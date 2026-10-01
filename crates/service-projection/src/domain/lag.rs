use super::{ProjectionCursor, ProjectionName};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ProjectionLag {
    pub error: String,
    #[schema(value_type = String)]
    pub projection: ProjectionName,
    #[schema(value_type = u64)]
    pub required_cursor: ProjectionCursor,
    #[schema(value_type = u64)]
    pub current_cursor: ProjectionCursor,
    pub retryable: bool,
    pub retry_after_seconds: u64,
}

impl ProjectionLag {
    pub fn new(
        projection: ProjectionName,
        required_cursor: ProjectionCursor,
        current_cursor: ProjectionCursor,
        retry_after_seconds: u64,
    ) -> Self {
        Self {
            error: "projection_lag".to_string(),
            projection,
            required_cursor,
            current_cursor,
            retryable: true,
            retry_after_seconds,
        }
    }
}

impl std::fmt::Display for ProjectionLag {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "projection {} is at cursor {}, requires {}",
            self.projection, self.current_cursor, self.required_cursor
        )
    }
}

impl std::error::Error for ProjectionLag {}

#[cfg(test)]
mod tests;
