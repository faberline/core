use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result};

use super::codec::{frame_end, HEADER_LEN};
use super::frame::{read_one_frame, scan_one_frame, LogFrame};

/// Reader for CRC-framed append logs.
pub struct FramedLogReader;

impl FramedLogReader {
    /// Visit validated frames without retaining the complete log in memory.
    ///
    /// The visitor runs in file order. A torn tail ends iteration at the last
    /// complete frame, matching `read_frames` recovery semantics.
    pub fn visit_frames(
        path: impl AsRef<Path>,
        from_seq: u64,
        mut visit: impl FnMut(LogFrame) -> Result<()>,
    ) -> Result<u64> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(0);
        }
        let mut file = File::open(path).with_context(|| format!("open log {}", path.display()))?;
        let total = file.metadata()?.len();
        let mut off = 0u64;
        let mut max_seq = 0u64;
        let mut header = [0u8; HEADER_LEN];
        loop {
            let Some((seq, payload, next)) = read_one_frame(&mut file, total, off, &mut header)?
            else {
                break;
            };
            if seq > from_seq {
                max_seq = max_seq.max(seq);
                visit(LogFrame { seq, payload })?;
            }
            off = next;
        }
        Ok(max_seq)
    }

    pub fn replay(
        path: impl AsRef<Path>,
        from_seq: u64,
        mut apply: impl FnMut(LogFrame),
    ) -> Result<u64> {
        Self::visit_frames(path, from_seq, |frame| {
            apply(frame);
            Ok(())
        })
    }

    pub fn read_frames(path: impl AsRef<Path>, from_seq: u64) -> Result<Vec<LogFrame>> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut file = File::open(path).with_context(|| format!("open log {}", path.display()))?;
        let total = file.metadata()?.len();
        let mut off = 0u64;
        let mut out = Vec::new();
        let mut header = [0u8; HEADER_LEN];
        loop {
            let Some((seq, payload, next)) = read_one_frame(&mut file, total, off, &mut header)?
            else {
                break;
            };
            if seq > from_seq {
                out.push(LogFrame { seq, payload });
            }
            off = next;
        }
        Ok(out)
    }

    /// Read at most `limit` frames after `from_seq` without retaining the
    /// skipped payloads. Callers that repeatedly page a validated open log use
    /// this to keep recovery memory bounded by one page.
    pub fn read_frames_bounded(
        path: impl AsRef<Path>,
        from_seq: u64,
        limit: usize,
    ) -> Result<Vec<LogFrame>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut file = File::open(path).with_context(|| format!("open log {}", path.display()))?;
        let total = file.metadata()?.len();
        let mut off = 0u64;
        let mut out = Vec::with_capacity(limit.min(1_000));
        let mut header = [0u8; HEADER_LEN];
        while off + HEADER_LEN as u64 <= total && out.len() < limit {
            file.seek(SeekFrom::Start(off))?;
            if file.read_exact(&mut header).is_err() {
                break;
            }
            let seq = u64::from_le_bytes(header[0..8].try_into().expect("fixed sequence bytes"));
            let len =
                u32::from_le_bytes(header[8..12].try_into().expect("fixed length bytes")) as u64;
            let crc = u32::from_le_bytes(header[12..16].try_into().expect("fixed crc bytes"));
            let Some(frame_end) = frame_end(off, len, total) else {
                break;
            };
            if seq > from_seq {
                let mut payload = vec![0u8; len as usize];
                if file.read_exact(&mut payload).is_err() || crc32fast::hash(&payload) != crc {
                    break;
                }
                out.push(LogFrame { seq, payload });
            }
            off = frame_end;
        }
        Ok(out)
    }

    pub fn scan_good_end(path: impl AsRef<Path>) -> Result<u64> {
        let path = path.as_ref();
        let mut file = File::open(path).with_context(|| format!("open log {}", path.display()))?;
        let total = file.metadata()?.len();
        let mut off = 0u64;
        let mut header = [0u8; HEADER_LEN];
        while let Some(next) = scan_one_frame(&mut file, total, off, &mut header)? {
            off = next;
        }
        Ok(off)
    }
}
