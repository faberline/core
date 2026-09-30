use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Result};

use crate::{sync_parent_dir, FsyncPolicy};

use super::observer::FramedLogTrimObserver;
use super::writer::FramedLogWriter;

/// An EverySec sync captured while the writer lock was held. The file sync is
/// done by [`FramedLogSyncPlan::sync_off_lock`] without holding that lock.
#[derive(Debug)]
pub struct FramedLogSyncPlan {
    file: File,
    path: PathBuf,
    revision: u64,
    identity: Arc<()>,
    observer: Option<Arc<dyn FramedLogTrimObserver>>,
}

impl FramedLogWriter {
    /// Prepare an EverySec sync while holding the writer lock.
    ///
    /// The returned file is pinned to the current inode. Callers must invoke
    /// the plan's `sync_off_lock` after releasing any external mutex, then
    /// [`FramedLogWriter::complete_sync`] after reacquiring it.
    pub fn begin_sync(&mut self) -> Result<Option<FramedLogSyncPlan>> {
        if self.policy != FsyncPolicy::EverySec
            || (self.trim_observer.is_none() && !self.dirty)
            || self.last_sync.elapsed() < self.sync_every
        {
            return Ok(None);
        }
        self.flush()?;
        Ok(Some(FramedLogSyncPlan {
            file: self
                .file
                .get_ref()
                .try_clone()
                .context("pin log for sync")?,
            path: self.path.clone(),
            revision: self.sync_revision,
            identity: Arc::clone(&self.sync_identity),
            observer: self.trim_observer.clone(),
        }))
    }

    /// Finish a split-phase sync after reacquiring the writer lock.
    pub fn complete_sync(&mut self, plan: FramedLogSyncPlan) -> Result<()> {
        if !Arc::ptr_eq(&plan.identity, &self.sync_identity) {
            bail!("sync plan belongs to another writer");
        }
        if self.sync_revision == plan.revision {
            self.last_sync = Instant::now();
            self.dirty = false;
        }
        Ok(())
    }
}

impl FramedLogSyncPlan {
    /// Sync the pinned file while the writer mutex is not held.
    pub fn sync_off_lock(&self) -> Result<()> {
        if let Some(observer) = &self.observer {
            observer.before_background_sync();
        }
        self.file.sync_all().context("fsync log")?;
        sync_parent_dir(&self.path)
    }
}
