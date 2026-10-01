use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{bail, Context, Result};

use crate::sync_parent_dir;

use super::observer::FramedLogTrimObserver;
use super::trim::remove_abandoned_trim_temp;
use super::trim_stream::stream_trim_range;
use super::writer::FramedLogWriter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrimPlanState {
    Prepared,
    Copying,
    Ready,
    Failed,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrimFileIdentity {
    device: u64,
    inode: u64,
}

/// One staged rewrite of a pinned log inode. The old log stays authoritative
/// while callers release the writer mutex to copy and sync its stable prefix.
/// Finalization includes every later append before replacing the live file.
/// This staged API requires Unix positional reads and stable inode identity.
#[derive(Debug)]
pub struct FramedLogTrimPlan {
    through: u64,
    source: File,
    source_identity: TrimFileIdentity,
    source_end: u64,
    tmp: PathBuf,
    temp: Option<BufWriter<File>>,
    identity: Arc<()>,
    revision: u64,
    id: u64,
    active: Arc<Mutex<Option<u64>>>,
    state: TrimPlanState,
    observer: Option<Arc<dyn FramedLogTrimObserver>>,
    #[cfg(test)]
    faults: Arc<TrimFaults>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TrimFaultPoint {
    PrefixTempSync,
    SuffixTempSync,
    ParentSyncAfterRename,
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(super) struct TrimFaults(Mutex<Option<TrimFaultPoint>>);

#[cfg(test)]
impl TrimFaults {
    pub(super) fn fail_once(&self, point: TrimFaultPoint) {
        *self.0.lock().unwrap() = Some(point);
    }

    fn check(&self, point: TrimFaultPoint) -> Result<()> {
        let mut next = self.0.lock().unwrap();
        if *next == Some(point) {
            *next = None;
            bail!("injected trim I/O failure: {point:?}");
        }
        Ok(())
    }
}

impl FramedLogWriter {
    /// Capture a complete source boundary while the caller owns the writer.
    /// Flush buffers here; scanning and syncing belong to the unlocked plan.
    pub fn begin_trim_mapped(&mut self, through: u64) -> Result<FramedLogTrimPlan> {
        let active = Arc::clone(&self.trim_active);
        let mut owner = active
            .lock()
            .map_err(|_| anyhow::anyhow!("trim ownership poisoned"))?;
        if owner.is_some() {
            bail!("trim plan Busy");
        }
        let id = self.trim_next_id;
        let next_id = id.checked_add(1).context("trim identity exhausted")?;
        self.flush()?;
        let source = self
            .file
            .get_ref()
            .try_clone()
            .context("pin live trim source")?;
        let metadata = source.metadata()?;
        let source_identity = trim_file_identity(&metadata)?;
        if trim_file_identity(&std::fs::symlink_metadata(&self.path)?)? != source_identity {
            bail!("trim source path changed");
        }
        let tmp = self.compact_tmp_path();
        remove_abandoned_trim_temp(&tmp)?;
        let temp = BufWriter::new(
            OpenOptions::new()
                .create_new(true)
                .read(true)
                .append(true)
                .open(&tmp)
                .with_context(|| format!("create log compaction temp {}", tmp.display()))?,
        );
        // Nothing fallible follows ownership publication. A failed open
        // cannot strand a phantom Busy plan.
        *owner = Some(id);
        self.trim_next_id = next_id;
        Ok(FramedLogTrimPlan {
            through,
            source,
            source_identity,
            source_end: metadata.len(),
            tmp,
            temp: Some(temp),
            identity: Arc::clone(&self.trim_identity),
            revision: self.trim_revision,
            id,
            active: Arc::clone(&active),
            state: TrimPlanState::Prepared,
            observer: self.trim_observer.clone(),
            #[cfg(test)]
            faults: Arc::clone(&self.trim_faults),
        })
    }

    /// Include the suffix appended during the unlocked copy, then publish.
    /// The caller owns the writer again. Suffix and directory sync can still
    /// wait for the filesystem; this does not impose a time bound.
    pub fn finish_trim_mapped(&mut self, mut plan: FramedLogTrimPlan) -> Result<()> {
        if plan.state != TrimPlanState::Ready
            || !Arc::ptr_eq(&plan.identity, &self.trim_identity)
            || plan.revision != self.trim_revision
            || *self
                .trim_active
                .lock()
                .map_err(|_| anyhow::anyhow!("trim ownership poisoned"))?
                != Some(plan.id)
        {
            bail!("trim plan is not publishable");
        }
        let revision = self
            .trim_revision
            .checked_add(1)
            .context("trim revision exhausted")?;
        if trim_file_identity(&self.file.get_ref().metadata()?)? != plan.source_identity
            || trim_file_identity(&plan.source.metadata()?)? != plan.source_identity
            || trim_file_identity(&std::fs::symlink_metadata(&self.path)?)? != plan.source_identity
        {
            bail!("trim source path changed");
        }
        self.flush()?;
        let current_end = self.file.get_ref().metadata()?.len();
        if current_end < plan.source_end {
            bail!("trim source shrank below captured boundary");
        }
        let temp = plan.temp.as_mut().context("missing trim temp")?;
        let added = stream_trim_range(
            &plan.source,
            plan.source_end,
            current_end,
            plan.through,
            temp,
            None,
        )?;
        temp.flush().context("flush log compaction suffix")?;
        if added {
            #[cfg(test)]
            plan.faults.check(TrimFaultPoint::SuffixTempSync)?;
            temp.get_ref()
                .sync_all()
                .context("fsync log compaction suffix")?;
        }
        if trim_file_identity(&temp.get_ref().metadata()?)?
            != trim_file_identity(&std::fs::symlink_metadata(&plan.tmp)?)?
        {
            bail!("trim temporary path changed");
        }
        let file = plan
            .temp
            .take()
            .context("missing trim temp")?
            .into_inner()
            .map_err(|error| error.into_error())
            .context("retain log compaction temp handle")?;
        std::fs::rename(&plan.tmp, &self.path).context("commit log compaction")?;
        self.file = BufWriter::new(file);
        self.trim_revision = revision;
        self.sync_revision = self
            .sync_revision
            .checked_add(1)
            .context("log sync revision exhausted")?;
        plan.state = TrimPlanState::Published;
        #[cfg(test)]
        plan.faults.check(TrimFaultPoint::ParentSyncAfterRename)?;
        sync_parent_dir(&self.path)?;
        self.dirty = false;
        self.last_sync = Instant::now();
        // Drop clears only this plan's ownership, also after a sync error.
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn trim_faults_for_test(&self) -> Arc<TrimFaults> {
        Arc::clone(&self.trim_faults)
    }
}

impl FramedLogTrimPlan {
    /// Validate and copy the whole captured prefix, then durably sync it.
    /// This has no writer borrow and never changes the live log.
    pub fn copy_stable_prefix(&mut self) -> Result<()> {
        if self.state != TrimPlanState::Prepared {
            self.state = TrimPlanState::Failed;
            bail!("trim plan is not prepared");
        }
        self.state = TrimPlanState::Copying;
        let result = (|| {
            let temp = self.temp.as_mut().context("missing trim temp")?;
            stream_trim_range(
                &self.source,
                0,
                self.source_end,
                self.through,
                temp,
                self.observer.as_deref(),
            )?;
            temp.flush().context("flush log compaction temp")?;
            if let Some(observer) = &self.observer {
                observer.before_temp_sync(self.through);
            }
            #[cfg(test)]
            self.faults.check(TrimFaultPoint::PrefixTempSync)?;
            temp.get_ref()
                .sync_all()
                .context("fsync log compaction temp")?;
            Ok(())
        })();
        self.state = if result.is_ok() {
            TrimPlanState::Ready
        } else {
            TrimPlanState::Failed
        };
        result
    }
}

impl Drop for FramedLogTrimPlan {
    fn drop(&mut self) {
        let mut owner = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *owner == Some(self.id) {
            // Keep ownership until removal completes, so the next begin
            // cannot create its temp between clearing this ID and unlinking.
            if let Some(temp) = self.temp.take() {
                // Discard unpublished buffered bytes without issuing another
                // write from Drop after an earlier I/O failure.
                drop(temp.into_parts());
            }
            if self.state != TrimPlanState::Published {
                let _ = std::fs::remove_file(&self.tmp);
            }
            *owner = None;
        }
    }
}

fn trim_file_identity(metadata: &std::fs::Metadata) -> Result<TrimFileIdentity> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("trim requires a regular nonsymlink file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(TrimFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    bail!("staged log trim requires Unix inode identity")
}
