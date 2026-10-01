use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::digest::sha256;
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
    fn apply_idempotent(&self, record: &Record) -> Result<()>;
    fn snapshot(&self) -> Result<Vec<u8>>;
    fn restore(&self, state: &[u8]) -> Result<()>;

    /// Finish work that is safe only after the projection snapshot is durable.
    ///
    /// Implementations use this for idempotent cleanup of files that the new
    /// snapshot no longer references. The default keeps existing consumers
    /// source compatible.
    fn checkpoint_committed(&self) -> Result<()> {
        Ok(())
    }

    fn semantic_digest(&self) -> Result<String> {
        Ok(sha256(&self.snapshot()?))
    }
}

pub(crate) fn validate_descriptor(descriptor: &ProjectionDescriptor) -> Result<()> {
    if descriptor.name.trim().is_empty()
        || descriptor.name.contains('/')
        || descriptor.name.contains('\0')
    {
        bail!("projection name is invalid");
    }
    Ok(())
}
