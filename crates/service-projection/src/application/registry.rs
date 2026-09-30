use std::{collections::BTreeMap, sync::Arc};

use anyhow::{bail, Context, Result};

use super::config::ProjectionRuntimeConfig;
use super::handle::ProjectionHandle;
use crate::domain::{
    Projection, ProjectionRecord, ProjectionSource, ProjectionStateStore, RebuildComparison,
};

trait ProjectionControl: Send + Sync {
    fn current_cursor(&self) -> u64;
    fn catch_up(&self) -> Result<u64>;
    fn semantic_digest(&self) -> Result<String>;
    fn rebuild_and_compare(&self) -> Result<RebuildComparison>;
    fn flush(&self) -> Result<()>;
}

impl<Record, P> ProjectionControl for ProjectionHandle<Record, P>
where
    Record: ProjectionRecord,
    P: Projection<Record>,
{
    fn current_cursor(&self) -> u64 {
        ProjectionHandle::current_cursor(self)
    }

    fn catch_up(&self) -> Result<u64> {
        ProjectionHandle::catch_up(self)
    }

    fn semantic_digest(&self) -> Result<String> {
        ProjectionHandle::semantic_digest(self)
    }

    fn rebuild_and_compare(&self) -> Result<RebuildComparison> {
        ProjectionHandle::rebuild_and_compare(self)
    }

    fn flush(&self) -> Result<()> {
        ProjectionHandle::flush(self)
    }
}

pub struct ProjectionRegistry<Record>
where
    Record: ProjectionRecord,
{
    store: Arc<dyn ProjectionStateStore>,
    source: Arc<dyn ProjectionSource<Record>>,
    config: ProjectionRuntimeConfig,
    controls: BTreeMap<String, Arc<dyn ProjectionControl>>,
}

impl<Record> ProjectionRegistry<Record>
where
    Record: ProjectionRecord,
{
    /// A registry that saves projection state through `store`, after
    /// preparing its root.
    pub(crate) fn from_store(
        store: Arc<dyn ProjectionStateStore>,
        source: Arc<dyn ProjectionSource<Record>>,
        config: ProjectionRuntimeConfig,
    ) -> Result<Self> {
        store.prepare_root()?;
        Ok(Self {
            store,
            source,
            config,
            controls: BTreeMap::new(),
        })
    }

    pub fn register<P, Factory>(
        &mut self,
        factory: Factory,
    ) -> Result<Arc<ProjectionHandle<Record, P>>>
    where
        P: Projection<Record>,
        Factory: Fn() -> Result<Arc<P>> + Send + Sync + 'static,
    {
        let handle = Arc::new(ProjectionHandle::open(
            self.store.clone(),
            self.source.clone(),
            Arc::new(factory),
            self.config,
        )?);
        let name = handle.descriptor().name;
        if self.controls.contains_key(&name) {
            bail!("projection {name} is registered more than once");
        }
        self.controls.insert(name, handle.clone());
        Ok(handle)
    }

    pub fn projection_names(&self) -> Vec<String> {
        self.controls.keys().cloned().collect()
    }

    pub fn has_projection(&self, name: &str) -> bool {
        self.controls.contains_key(name)
    }

    pub fn current_cursor(&self, name: &str) -> Result<u64> {
        Ok(self.control(name)?.current_cursor())
    }

    pub fn catch_up(&self, name: &str) -> Result<u64> {
        self.control(name)?.catch_up()
    }

    pub fn semantic_digest(&self, name: &str) -> Result<String> {
        self.control(name)?.semantic_digest()
    }

    pub fn rebuild_and_compare(&self, name: &str) -> Result<RebuildComparison> {
        self.control(name)?.rebuild_and_compare()
    }

    pub fn flush_all(&self) -> Result<()> {
        for control in self.controls.values() {
            control.flush()?;
        }
        Ok(())
    }

    fn control(&self, name: &str) -> Result<&Arc<dyn ProjectionControl>> {
        self.controls
            .get(name)
            .with_context(|| format!("unknown projection {name}"))
    }
}
