use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use anyhow::{bail, Context, Result};
use memmap2::{Mmap, MmapOptions};

use super::codec::{
    complete_frame_end, validate_payload_crc_streaming, HEADER_LEN, MAX_FRAME_PAYLOAD_BYTES,
};

/// One validated log frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFrame {
    pub seq: u64,
    pub payload: Vec<u8>,
}

/// A validated read-only frame view.
///
/// The completed implementation owns its file mapping, so this view remains
/// valid after its cursor is dropped or the original path is replaced.
#[derive(Debug)]
pub struct MappedLogFrame {
    pub seq: u64,
    payload: Option<Mmap>,
}

impl MappedLogFrame {
    pub fn payload(&self) -> &[u8] {
        self.payload.as_deref().unwrap_or_default()
    }
}

pub(super) fn read_one_frame(
    file: &mut File,
    total: u64,
    off: u64,
    header: &mut [u8; HEADER_LEN],
) -> Result<Option<(u64, Vec<u8>, u64)>> {
    if off + HEADER_LEN as u64 > total {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(off))?;
    if file.read_exact(header).is_err() {
        return Ok(None);
    }
    let seq = u64::from_le_bytes([
        header[0], header[1], header[2], header[3], header[4], header[5], header[6], header[7],
    ]);
    let len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as u64;
    let crc = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
    let Some(frame_end) = complete_frame_end(off, len, total) else {
        if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
            bail!(
                "oversized legacy log frame at byte {off} is incomplete; refusing destructive recovery"
            );
        }
        return Ok(None);
    };
    if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
        validate_payload_crc_streaming(file, len, crc)
            .with_context(|| format!("validate oversized legacy log frame at byte {off}"))?;
        bail!(
            "validated legacy log frame at byte {off} has {len} payload bytes, above the supported read limit {MAX_FRAME_PAYLOAD_BYTES}; the file was not modified"
        );
    }
    let mut payload = vec![0u8; len as usize];
    if file.read_exact(&mut payload).is_err() {
        return Ok(None);
    }
    if crc32fast::hash(&payload) != crc {
        return Ok(None);
    }
    Ok(Some((seq, payload, frame_end)))
}

/// Read and validate one frame without allocating an owned payload buffer.
///
/// The mapping refers to the cursor's already-open file descriptor. Supported
/// writers either append beyond the cursor's pinned `total` length or atomically
/// replace the path with a different inode; neither operation truncates or
/// changes the mapped range of this original inode while a view is alive.
pub(super) fn read_one_mapped_frame(
    file: &mut File,
    total: u64,
    off: u64,
    header: &mut [u8; HEADER_LEN],
) -> Result<Option<(MappedLogFrame, u64)>> {
    read_one_mapped_frame_with_limit(file, total, off, header, false)
}

pub(super) fn read_one_large_mapped_frame(
    file: &mut File,
    total: u64,
    off: u64,
    header: &mut [u8; HEADER_LEN],
) -> Result<Option<(MappedLogFrame, u64)>> {
    read_one_mapped_frame_with_limit(file, total, off, header, true)
}

fn read_one_mapped_frame_with_limit(
    file: &mut File,
    total: u64,
    off: u64,
    header: &mut [u8; HEADER_LEN],
    accept_large: bool,
) -> Result<Option<(MappedLogFrame, u64)>> {
    if off
        .checked_add(HEADER_LEN as u64)
        .is_none_or(|end| end > total)
    {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(off))?;
    if file.read_exact(header).is_err() {
        return Ok(None);
    }
    let seq = u64::from_le_bytes([
        header[0], header[1], header[2], header[3], header[4], header[5], header[6], header[7],
    ]);
    let len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as u64;
    let crc = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
    let Some(frame_end) = complete_frame_end(off, len, total) else {
        if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
            bail!(
                "oversized legacy log frame at byte {off} is incomplete; refusing destructive recovery"
            );
        }
        return Ok(None);
    };
    if len > MAX_FRAME_PAYLOAD_BYTES as u64 && !accept_large {
        validate_payload_crc_streaming(file, len, crc)
            .with_context(|| format!("validate oversized legacy log frame at byte {off}"))?;
        bail!(
            "validated legacy log frame at byte {off} has {len} payload bytes, above the supported read limit {MAX_FRAME_PAYLOAD_BYTES}; the file was not modified"
        );
    }
    if let Err(error) = validate_payload_crc_streaming(file, len, crc) {
        if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
            return Err(error).with_context(|| {
                format!("oversized legacy log frame at byte {off} failed validation; refusing destructive recovery")
            });
        }
        return Ok(None);
    }
    let payload = if len == 0 {
        None
    } else {
        let payload_offset = off
            .checked_add(HEADER_LEN as u64)
            .expect("validated frame payload offset fits u64");
        let payload_len = usize::try_from(len).expect("validated frame payload length fits usize");
        // SAFETY: `payload_offset..frame_end` is within the cursor's immutable
        // open-time length and its CRC has just been checked. The supported
        // writer operations append after that length or atomically replace the
        // path, which leaves this pinned inode and mapped byte range intact.
        Some(unsafe {
            MmapOptions::new()
                .offset(payload_offset)
                .len(payload_len)
                .map(&*file)
                .with_context(|| format!("map validated log frame at byte {off}"))?
        })
    };
    Ok(Some((MappedLogFrame { seq, payload }, frame_end)))
}

/// Scan one frame without allocating its payload.
///
/// New writers cap frames at [`MAX_FRAME_PAYLOAD_BYTES`]. Older releases
/// accepted every `u32` length. Recovery therefore streams the CRC for a
/// complete legacy frame. An incomplete or corrupt oversized frame is an
/// error, not a torn tail, because truncating it could delete valid data from
/// an older writer.
pub(super) fn scan_one_frame(
    file: &mut File,
    total: u64,
    off: u64,
    header: &mut [u8; HEADER_LEN],
) -> Result<Option<u64>> {
    if off + HEADER_LEN as u64 > total {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(off))?;
    if file.read_exact(header).is_err() {
        return Ok(None);
    }
    let len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as u64;
    let crc = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
    let Some(end) = complete_frame_end(off, len, total) else {
        if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
            bail!(
                "oversized legacy log frame at byte {off} is incomplete; refusing destructive recovery"
            );
        }
        return Ok(None);
    };
    match validate_payload_crc_streaming(file, len, crc) {
        Ok(()) => Ok(Some(end)),
        Err(error) if len > MAX_FRAME_PAYLOAD_BYTES as u64 => Err(error).with_context(|| {
            format!(
                "oversized legacy log frame at byte {off} failed validation; refusing destructive recovery"
            )
        }),
        Err(_) => Ok(None),
    }
}
