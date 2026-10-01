use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Result};

use crate::sync_parent_dir;

use super::codec::write_frame;
#[cfg(any(not(unix), test))]
use super::codec::write_large_frame;
use super::cursor::FramedLogCursor;
use super::writer::FramedLogWriter;

impl FramedLogWriter {
    pub fn truncate_through(&mut self, through: u64) -> Result<()> {
        let active = Arc::clone(&self.trim_active);
        let owner = active
            .lock()
            .map_err(|_| anyhow::anyhow!("trim ownership poisoned"))?;
        if owner.is_some() {
            bail!("trim plan Busy");
        }
        let revision = self
            .trim_revision
            .checked_add(1)
            .context("trim revision exhausted")?;
        self.flush()?;
        let mut frames = FramedLogCursor::open(&self.path)?;
        let tmp = self.compact_tmp_path();
        remove_abandoned_trim_temp(&tmp)?;
        let mut dst = BufWriter::new(
            OpenOptions::new()
                .create_new(true)
                .read(true)
                .append(true)
                .open(&tmp)
                .with_context(|| format!("create log compaction temp {}", tmp.display()))?,
        );
        // Preserve the existing owned-frame payload limit on this separate API.
        while let Some(frame) = frames.next_frame()? {
            if frame.seq > through {
                write_frame(&mut dst, frame.seq, &frame.payload)?;
            }
        }
        dst.flush().context("flush log compaction temp")?;
        dst.get_ref()
            .sync_all()
            .context("fsync log compaction temp")?;
        let file = dst
            .into_inner()
            .map_err(|error| error.into_error())
            .context("retain log compaction temp handle")?;
        std::fs::rename(&tmp, &self.path).context("commit log compaction")?;
        // Keep the published handle even if the directory sync fails.
        self.file = BufWriter::new(file);
        self.trim_revision = revision;
        self.sync_revision = self
            .sync_revision
            .checked_add(1)
            .context("log sync revision exhausted")?;
        sync_parent_dir(&self.path)?;
        self.dirty = false;
        self.last_sync = Instant::now();
        Ok(())
    }

    /// Rewrite complete CRC-valid u32-sized frames through the cut.
    /// Unix callers may split begin/copy/finish to release an external mutex.
    /// Other platforms retain the existing one-phase mapped rewrite.
    pub fn truncate_through_mapped(&mut self, through: u64) -> Result<()> {
        #[cfg(unix)]
        {
            let mut plan = self.begin_trim_mapped(through)?;
            plan.copy_stable_prefix()?;
            self.finish_trim_mapped(plan)
        }
        #[cfg(not(unix))]
        {
            self.truncate_mapped_in_place(through)
        }
    }

    // Keep the established synchronous API on platforms without Unix inode
    // identity. Tests also exercise this implementation on the build host.
    #[cfg(any(not(unix), test))]
    pub(super) fn truncate_mapped_in_place(&mut self, through: u64) -> Result<()> {
        let active = Arc::clone(&self.trim_active);
        let owner = active
            .lock()
            .map_err(|_| anyhow::anyhow!("trim ownership poisoned"))?;
        if owner.is_some() {
            bail!("trim plan Busy");
        }
        let revision = self
            .trim_revision
            .checked_add(1)
            .context("trim revision exhausted")?;
        self.flush()?;
        let mut frames = FramedLogCursor::open(&self.path)?;
        let tmp = self.compact_tmp_path();
        remove_abandoned_trim_temp(&tmp)?;
        let file = {
            let mut dst = BufWriter::new(
                OpenOptions::new()
                    .create_new(true)
                    .read(true)
                    .append(true)
                    .open(&tmp)
                    .with_context(|| format!("create log compaction temp {}", tmp.display()))?,
            );
            while let Some(frame) = frames.next_large_mapped_frame()? {
                if frame.seq <= through {
                    if let Some(observer) = &self.trim_observer {
                        observer.covered_frame(through, frame.seq);
                    }
                    continue;
                }
                write_large_frame(&mut dst, frame.seq, frame.payload())?;
            }
            dst.flush().context("flush log compaction temp")?;
            if let Some(observer) = &self.trim_observer {
                observer.before_temp_sync(through);
            }
            dst.get_ref()
                .sync_all()
                .context("fsync log compaction temp")?;
            dst.into_inner()
                .map_err(|error| error.into_error())
                .context("retain log compaction temp handle")?
        };
        std::fs::rename(&tmp, &self.path).with_context(|| {
            format!(
                "commit log compaction {} -> {}",
                tmp.display(),
                self.path.display()
            )
        })?;
        // Install the staged inode before the directory fsync. If that fsync
        // reports an error, later appends still target the published log.
        self.file = BufWriter::new(file);
        self.trim_revision = revision;
        self.sync_revision = self
            .sync_revision
            .checked_add(1)
            .context("log sync revision exhausted")?;
        sync_parent_dir(&self.path)?;
        self.dirty = false;
        self.last_sync = Instant::now();
        Ok(())
    }

    pub(super) fn compact_tmp_path(&self) -> PathBuf {
        let mut tmp = self.path.as_os_str().to_os_string();
        tmp.push(".compact.tmp");
        tmp.into()
    }
}

pub(super) fn remove_abandoned_trim_temp(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove abandoned log compaction temp"),
    }
}
