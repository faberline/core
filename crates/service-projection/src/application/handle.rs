use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;
use chrono::Utc;
use tokio::sync::Notify;

use super::config::ProjectionRuntimeConfig;
use crate::domain::{
    checkpoint, validate_descriptor, Projection, ProjectionCheckpoint, ProjectionDescriptor,
    ProjectionError, ProjectionLag, ProjectionRecord, ProjectionSource, ProjectionStateStore,
    RebuildComparison,
};

mod locked;

struct LiveProjection<P> {
    implementation: Arc<P>,
    checkpoint: ProjectionCheckpoint,
    persisted_cursor: u64,
}

type ProjectionFactory<P> = Arc<dyn Fn() -> Result<Arc<P>> + Send + Sync>;

pub struct ProjectionHandle<Record, P>
where
    Record: ProjectionRecord,
    P: Projection<Record>,
{
    source: Arc<dyn ProjectionSource<Record>>,
    factory: ProjectionFactory<P>,
    store: Arc<dyn ProjectionStateStore>,
    name: String,
    live: Mutex<LiveProjection<P>>,
    published: Notify,
    config: ProjectionRuntimeConfig,
}

impl<Record, P> ProjectionHandle<Record, P>
where
    Record: ProjectionRecord,
    P: Projection<Record>,
{
    pub(super) fn open(
        store: Arc<dyn ProjectionStateStore>,
        source: Arc<dyn ProjectionSource<Record>>,
        factory: ProjectionFactory<P>,
        config: ProjectionRuntimeConfig,
    ) -> Result<Self> {
        let mut implementation = factory()?;
        let descriptor = implementation.descriptor();
        validate_descriptor(&descriptor)?;
        let (checkpoint, restored, rebuild_invalid_snapshot) = match store
            .read(descriptor.name())?
        {
            Some(bytes) => {
                match restore_saved(store.as_ref(), &descriptor, implementation.as_ref(), &bytes) {
                    Ok(checkpoint) => (checkpoint, true, false),
                    Err(_) => {
                        store.quarantine(descriptor.name(), &bytes)?;
                        implementation = factory()?;
                        (
                            ProjectionCheckpoint::empty(&descriptor, Utc::now()),
                            false,
                            true,
                        )
                    }
                }
            }
            None => (
                ProjectionCheckpoint::empty(&descriptor, Utc::now()),
                false,
                false,
            ),
        };
        let handle = Self {
            source,
            factory,
            store,
            name: descriptor.name().to_string(),
            live: Mutex::new(LiveProjection {
                implementation,
                persisted_cursor: checkpoint.cursor,
                checkpoint,
            }),
            published: Notify::new(),
            config,
        };
        if restored {
            handle.projection().checkpoint_committed()?;
        }
        if rebuild_invalid_snapshot
            || handle.current_source_generation() != handle.source.generation()
        {
            handle.rebuild()?;
        }
        Ok(handle)
    }

    pub fn projection(&self) -> Arc<P> {
        self.live
            .lock()
            .expect("projection state lock poisoned")
            .implementation
            .clone()
    }

    pub fn descriptor(&self) -> ProjectionDescriptor {
        self.projection().descriptor()
    }

    pub fn current_cursor(&self) -> u64 {
        self.live
            .lock()
            .expect("projection state lock poisoned")
            .checkpoint
            .cursor
    }

    pub fn semantic_digest(&self) -> Result<String> {
        Ok(self.projection().semantic_digest()?)
    }

    pub fn catch_up(&self) -> Result<u64> {
        let target = self.source.current_cursor();
        let generation = self.source.generation();
        let mut live = self.live.lock().expect("projection state lock poisoned");
        if live.checkpoint.source_generation != generation {
            return Ok(self
                .rebuild_locked(&mut live, target, generation)?
                .rebuilt_cursor);
        }
        self.catch_up_locked(&mut live, target, generation)
    }

    pub async fn wait_for_min_cursor(
        &self,
        required_cursor: u64,
        timeout: Duration,
    ) -> std::result::Result<u64, ProjectionLag> {
        let started = Instant::now();
        loop {
            let published = self.published.notified();
            let current = self.current_cursor();
            if current >= required_cursor {
                return Ok(current);
            }
            let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                return Err(self.lag(required_cursor, current));
            };
            if tokio::time::timeout(remaining, published).await.is_err() {
                return Err(self.lag(required_cursor, self.current_cursor()));
            }
        }
    }

    pub fn rebuild_and_compare(&self) -> Result<RebuildComparison> {
        let source_cursor = self.source.current_cursor();
        let source_generation = self.source.generation();
        {
            let mut live = self.live.lock().expect("projection state lock poisoned");
            if live.checkpoint.source_generation != source_generation {
                self.rebuild_locked(&mut live, source_cursor, source_generation)?;
            } else {
                self.catch_up_locked(&mut live, source_cursor, source_generation)?;
            }
        }
        let live_digest = self.semantic_digest()?;
        let (rebuilt, last_event_id) = self.build_projection(source_cursor)?;
        let rebuilt_digest = rebuilt.semantic_digest()?;
        let equal = live_digest == rebuilt_digest;
        if equal {
            let descriptor = rebuilt.descriptor();
            let state = rebuilt.snapshot()?;
            let checkpoint = checkpoint(
                &descriptor,
                source_cursor,
                source_generation,
                last_event_id,
                &state,
                Utc::now(),
            );
            self.store.persist(&self.name, &checkpoint, &state)?;
            rebuilt.checkpoint_committed()?;
            let mut live = self.live.lock().expect("projection state lock poisoned");
            live.implementation = rebuilt;
            live.checkpoint = checkpoint;
            live.persisted_cursor = source_cursor;
            self.published.notify_waiters();
        }
        Ok(RebuildComparison {
            source_cursor,
            rebuilt_cursor: source_cursor,
            live_digest,
            rebuilt_digest,
            equal,
        })
    }

    /// Rebuild from the current retained source and publish it even when the
    /// prior live projection differs. This is the required path after a source
    /// generation change such as retention or index repair.
    pub fn rebuild(&self) -> Result<RebuildComparison> {
        let source_cursor = self.source.current_cursor();
        let source_generation = self.source.generation();
        let mut live = self.live.lock().expect("projection state lock poisoned");
        self.rebuild_locked(&mut live, source_cursor, source_generation)
    }

    pub fn flush(&self) -> Result<()> {
        let target = self.source.current_cursor();
        let generation = self.source.generation();
        let mut live = self.live.lock().expect("projection state lock poisoned");
        if live.checkpoint.source_generation != generation {
            self.rebuild_locked(&mut live, target, generation)?;
        } else {
            self.catch_up_locked(&mut live, target, generation)?;
        }
        if live.checkpoint.cursor != live.persisted_cursor {
            self.persist_live(&mut live, generation)?;
        }
        Ok(())
    }
}

/// Decode saved state and restore it into `implementation`; an error means
/// the saved state is quarantined and the projection rebuilt.
fn restore_saved<Record, P>(
    store: &dyn ProjectionStateStore,
    descriptor: &ProjectionDescriptor,
    implementation: &P,
    bytes: &[u8],
) -> std::result::Result<ProjectionCheckpoint, ProjectionError>
where
    Record: ProjectionRecord,
    P: Projection<Record>,
{
    let (checkpoint, state) = store.restore(descriptor, bytes)?;
    implementation.restore(&state)?;
    Ok(checkpoint)
}
