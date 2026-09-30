use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::digest::sha256;
use super::projection_error::ProjectionError;
use super::source::ProjectionRecord;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ProjectionDescriptor {
    pub name: String,
    pub schema_version: u32,
    pub retention: String,
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
    if descriptor.name.trim().is_empty()
        || descriptor.name.contains('/')
        || descriptor.name.contains('\0')
    {
        return Err(ProjectionError::InvalidName);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
