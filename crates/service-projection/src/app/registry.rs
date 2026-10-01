use std::{path::Path, sync::Arc};

use anyhow::Result;

use crate::application::{ProjectionRegistry, ProjectionRuntimeConfig};
use crate::domain::{ProjectionRecord, ProjectionSource};
use crate::infrastructure::FileProjectionStateStore;

impl<Record> ProjectionRegistry<Record>
where
    Record: ProjectionRecord,
{
    pub fn new(
        root: impl AsRef<Path>,
        source: Arc<dyn ProjectionSource<Record>>,
        config: ProjectionRuntimeConfig,
    ) -> Result<Self> {
        Self::from_store(
            Arc::new(FileProjectionStateStore::new(root.as_ref())),
            source,
            config,
        )
    }
}
