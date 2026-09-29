use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::{strict_sync_parent_dir, sync_parent_dir, FsyncPolicy};

use super::codec::{checked_legacy_payload_len, checked_payload_len, HEADER_LEN};
use super::observer::FramedLogTrimObserver;
use super::reader::FramedLogReader;
#[cfg(test)]
use super::trim_plan::TrimFaults;

/// CRC-framed append log with clean torn-tail recovery.
pub struct FramedLogWriter {
    pub(super) path: PathBuf,
    pub(super) file: BufWriter<File>,
    pub(super) policy: FsyncPolicy,
    pub(super) last_sync: Instant,
    pub(super) sync_every: Duration,
    pub(super) dirty: bool,
    pub(super) sync_revision: u64,
    pub(super) sync_identity: Arc<()>,
    pub(super) trim_observer: Option<Arc<dyn FramedLogTrimObserver>>,
    pub(super) trim_identity: Arc<()>,
    pub(super) trim_revision: u64,
    pub(super) trim_active: Arc<Mutex<Option<u64>>>,
    pub(super) trim_next_id: u64,
    #[cfg(test)]
    pub(super) trim_faults: Arc<TrimFaults>,
}

impl FramedLogWriter {
    pub fn open(path: impl Into<PathBuf>, policy: FsyncPolicy) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create log dir {}", parent.display()))?;
            }
        }

        let good_end = if path.exists() {
            FramedLogReader::scan_good_end(&path)?
        } else {
            0
        };
        if path.exists() {
            let file = OpenOptions::new()
                .write(true)
                .open(&path)
                .with_context(|| format!("open log for truncate {}", path.display()))?;
            file.set_len(good_end)
                .with_context(|| format!("truncate log tail {}", path.display()))?;
            if policy != FsyncPolicy::Os {
                file.sync_all()
                    .with_context(|| format!("fsync truncated log {}", path.display()))?;
                sync_parent_dir(&path)?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open log for append {}", path.display()))?;
        Ok(Self {
            path,
            file: BufWriter::new(file),
            policy,
            last_sync: Instant::now(),
            sync_every: Duration::from_secs(1),
            dirty: false,
            sync_revision: 0,
            sync_identity: Arc::new(()),
            trim_observer: None,
            trim_identity: Arc::new(()),
            trim_revision: 0,
            trim_active: Arc::new(Mutex::new(None)),
            trim_next_id: 1,
            #[cfg(test)]
            trim_faults: Arc::new(TrimFaults::default()),
        })
    }

    #[doc(hidden)]
    pub fn with_trim_observer(mut self, observer: Arc<dyn FramedLogTrimObserver>) -> Self {
        self.trim_observer = Some(observer);
        // An observer is used by the checkpoint integration path to observe
        // the split-phase background sync.  A writer opened with the legacy
        // `Always` policy otherwise never creates a sync plan, so the hook
        // would be unreachable even though the observer is propagated into
        // the plan.  Keep the normal default unchanged and opt this observed
        // writer into the existing EverySec background lifecycle.
        self.policy = FsyncPolicy::EverySec;
        self
    }

    pub fn append(&mut self, seq: u64, payload: &[u8]) -> Result<()> {
        let len = checked_payload_len(payload.len())?;
        self.append_validated(seq, len, payload)
    }

    /// Append one format-valid frame without imposing the owned-frame limit.
    ///
    /// This is for callers that retain the payload in borrowed storage, such
    /// as an mmap. It still refuses lengths that the u32 frame format cannot
    /// represent.
    pub fn append_large_payload(&mut self, seq: u64, payload: &[u8]) -> Result<()> {
        let len = checked_legacy_payload_len(payload.len())?;
        self.append_validated(seq, len, payload)
    }

    fn append_validated(&mut self, seq: u64, len: u32, payload: &[u8]) -> Result<()> {
        let crc = crc32fast::hash(payload);
        let mut header = [0u8; HEADER_LEN];
        header[0..8].copy_from_slice(&seq.to_le_bytes());
        header[8..12].copy_from_slice(&len.to_le_bytes());
        header[12..16].copy_from_slice(&crc.to_le_bytes());
        self.file.write_all(&header).context("write log header")?;
        self.file.write_all(payload).context("write log payload")?;
        self.dirty = true;
        self.sync_revision = self
            .sync_revision
            .checked_add(1)
            .context("log sync revision exhausted")?;
        if self.policy.should_sync_immediately() {
            self.sync()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.file.flush().context("flush log writer")
    }

    pub fn sync(&mut self) -> Result<()> {
        self.flush()?;
        self.file.get_ref().sync_all().context("fsync log")?;
        sync_parent_dir(&self.path)?;
        self.last_sync = Instant::now();
        self.dirty = false;
        Ok(())
    }

    /// Flush and fsync the log, then require a successful parent-directory
    /// fsync. This is the pre-commit boundary used by durable replacement.
    pub fn sync_strict(&mut self) -> Result<()> {
        self.flush()?;
        self.file.get_ref().sync_all().context("strict fsync log")?;
        strict_sync_parent_dir(&self.path)?;
        self.last_sync = Instant::now();
        self.dirty = false;
        Ok(())
    }

    pub fn maybe_sync(&mut self) -> Result<()> {
        if self.policy != FsyncPolicy::EverySec {
            return Ok(());
        }
        if self.dirty && self.last_sync.elapsed() >= self.sync_every {
            self.sync()?;
        }
        Ok(())
    }
}
