use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};

use super::codec::HEADER_LEN;
#[cfg(doc)]
use super::codec::MAX_FRAME_PAYLOAD_BYTES;
use super::frame::{
    read_one_frame, read_one_large_mapped_frame, read_one_mapped_frame, LogFrame, MappedLogFrame,
};

/// Stateful reader for one validated frame at a time.
///
/// The cursor keeps its byte offset. Repeated calls do not scan the skipped
/// prefix again, and memory is bounded by the current frame payload.
pub struct FramedLogCursor {
    file: Option<File>,
    total: u64,
    offset: u64,
    header: [u8; HEADER_LEN],
}

impl FramedLogCursor {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self {
                file: None,
                total: 0,
                offset: 0,
                header: [0; HEADER_LEN],
            });
        }
        let file = File::open(path).with_context(|| format!("open log {}", path.display()))?;
        let total = file.metadata()?.len();
        Ok(Self {
            file: Some(file),
            total,
            offset: 0,
            header: [0; HEADER_LEN],
        })
    }

    pub fn next_frame(&mut self) -> Result<Option<LogFrame>> {
        let Some(file) = self.file.as_mut() else {
            return Ok(None);
        };
        let Some((seq, payload, next)) =
            read_one_frame(file, self.total, self.offset, &mut self.header)?
        else {
            return Ok(None);
        };
        self.offset = next;
        Ok(Some(LogFrame { seq, payload }))
    }

    /// Return the next validated frame without creating an owned payload Vec.
    pub fn next_mapped_frame(&mut self) -> Result<Option<MappedLogFrame>> {
        let Some(file) = self.file.as_mut() else {
            return Ok(None);
        };
        let Some((frame, next)) =
            read_one_mapped_frame(file, self.total, self.offset, &mut self.header)?
        else {
            return Ok(None);
        };
        self.offset = next;
        Ok(Some(frame))
    }

    /// Return the next CRC-valid frame as a borrowed mapping, including
    /// legacy payloads above [`MAX_FRAME_PAYLOAD_BYTES`].
    pub fn next_large_mapped_frame(&mut self) -> Result<Option<MappedLogFrame>> {
        let Some(file) = self.file.as_mut() else {
            return Ok(None);
        };
        let Some((frame, next)) =
            read_one_large_mapped_frame(file, self.total, self.offset, &mut self.header)?
        else {
            return Ok(None);
        };
        self.offset = next;
        Ok(Some(frame))
    }

    /// Read a validated frame through this cursor's pinned file and length.
    /// The logical offset is unchanged, including when the path was replaced.
    pub fn reread_frame_at(&mut self, offset: u64) -> Result<Option<LogFrame>> {
        if offset
            .checked_add(HEADER_LEN as u64)
            .is_none_or(|end| end > self.total)
        {
            return Ok(None);
        }
        let Some(file) = self.file.as_mut() else {
            return Ok(None);
        };
        read_one_frame(file, self.total, offset, &mut self.header)
            .map(|frame| frame.map(|(seq, payload, _)| LogFrame { seq, payload }))
    }

    /// Reread one mapped frame without changing the replay offset.
    pub fn reread_mapped_frame_at(&mut self, offset: u64) -> Result<Option<MappedLogFrame>> {
        if offset
            .checked_add(HEADER_LEN as u64)
            .is_none_or(|end| end > self.total)
        {
            return Ok(None);
        }
        let Some(file) = self.file.as_mut() else {
            return Ok(None);
        };
        read_one_mapped_frame(file, self.total, offset, &mut self.header)
            .map(|frame| frame.map(|(frame, _)| frame))
    }

    /// Reread a large mapped frame from this cursor's pinned inode and
    /// open-time length without changing the replay offset.
    pub fn reread_large_mapped_frame_at(&mut self, offset: u64) -> Result<Option<MappedLogFrame>> {
        if offset
            .checked_add(HEADER_LEN as u64)
            .is_none_or(|end| end > self.total)
        {
            return Ok(None);
        }
        let Some(file) = self.file.as_mut() else {
            return Ok(None);
        };
        read_one_large_mapped_frame(file, self.total, offset, &mut self.header)
            .map(|frame| frame.map(|(frame, _)| frame))
    }

    pub fn byte_offset(&self) -> u64 {
        self.offset
    }
}
