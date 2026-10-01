use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::digest::sha256;
use super::projection_error::ProjectionError;
use super::source::ProjectionRecord;
use super::ProjectionName;

/// A projection's name, schema version and retention label. `try_new` checks
/// the name; `Deserialize` fills the fields directly, and the runtime checks
/// the name again when it opens the projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ProjectionDescriptor {
    #[schema(value_type = String)]
    name: ProjectionName,
    schema_version: u32,
    retention: String,
}

impl ProjectionDescriptor {
    /// A descriptor, or `ProjectionError::InvalidName` when the name is
    /// blank or contains `/` or NUL.
    pub fn try_new(
        name: ProjectionName,
        schema_version: u32,
        retention: impl Into<String>,
    ) -> Result<Self, ProjectionError> {
        let descriptor = Self {
            name,
            schema_version,
            retention: retention.into(),
        };
        validate_descriptor(&descriptor)?;
        Ok(descriptor)
    }

    pub fn name(&self) -> &ProjectionName {
        &self.name
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// A label the runtime does not interpret.
    pub fn retention(&self) -> &str {
        &self.retention
    }
}

pub trait Projection<Record>: Send + Sync + 'static
where
    Record: ProjectionRecord,
{
    fn descriptor(&self) -> ProjectionDescriptor;
    fn apply_idempotent(&self, record: &Record) -> Result<(), ProjectionError>;
    fn snapshot(&self) -> Result<Vec<u8>, ProjectionError>;
    fn restore(&self, state: &[u8]) -> Result<(), ProjectionError>;

    /// Finish work that is safe only after the projection snapshot is durable.
    ///
    /// Implementations use this for idempotent cleanup of files that the new
    /// snapshot no longer references. The default keeps existing consumers
    /// source compatible.
    fn checkpoint_committed(&self) -> Result<(), ProjectionError> {
        Ok(())
    }

    fn semantic_digest(&self) -> Result<String, ProjectionError> {
        Ok(sha256(&self.snapshot()?))
    }
}

pub(crate) fn validate_descriptor(
    descriptor: &ProjectionDescriptor,
) -> Result<(), ProjectionError> {
    if descriptor.name.as_str().trim().is_empty()
        || descriptor.name.as_str().contains('/')
        || descriptor.name.as_str().contains('\0')
    {
        return Err(ProjectionError::InvalidName);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
