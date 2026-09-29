use std::fs::File;
use std::io::{Read, Write};

use anyhow::{bail, Context, Result};

pub(super) const HEADER_LEN: usize = 16;

/// Largest payload accepted by the shared framed-log format.
///
/// This bound prevents a corrupt or hostile header from forcing a multi-GiB
/// allocation during recovery. Callers must split larger logical records.
pub const MAX_FRAME_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn write_frame(mut writer: impl Write, seq: u64, payload: &[u8]) -> Result<()> {
    let len = checked_payload_len(payload.len())?;
    write_frame_with_len(&mut writer, seq, len, payload)
}

#[cfg(any(not(unix), test))]
pub(super) fn write_large_frame(mut writer: impl Write, seq: u64, payload: &[u8]) -> Result<()> {
    let len = checked_legacy_payload_len(payload.len())?;
    write_frame_with_len(&mut writer, seq, len, payload)
}

fn write_frame_with_len(mut writer: impl Write, seq: u64, len: u32, payload: &[u8]) -> Result<()> {
    let crc = crc32fast::hash(payload);
    let mut header = [0u8; HEADER_LEN];
    header[0..8].copy_from_slice(&seq.to_le_bytes());
    header[8..12].copy_from_slice(&len.to_le_bytes());
    header[12..16].copy_from_slice(&crc.to_le_bytes());
    writer.write_all(&header).context("write log header")?;
    writer.write_all(payload).context("write log payload")?;
    Ok(())
}

pub(super) fn checked_payload_len(len: usize) -> Result<u32> {
    if len > MAX_FRAME_PAYLOAD_BYTES {
        anyhow::bail!("log payload {len} bytes exceeds maximum {MAX_FRAME_PAYLOAD_BYTES} bytes");
    }
    u32::try_from(len).context("log payload too large for u32 len")
}

pub(super) fn checked_legacy_payload_len(len: usize) -> Result<u32> {
    u32::try_from(len).context("log payload too large for u32 len")
}

pub(super) fn frame_end(off: u64, len: u64, total: u64) -> Option<u64> {
    if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
        return None;
    }
    complete_frame_end(off, len, total)
}

pub(super) fn complete_frame_end(off: u64, len: u64, total: u64) -> Option<u64> {
    let end = off.checked_add(HEADER_LEN as u64)?.checked_add(len)?;
    (end <= total).then_some(end)
}

pub(super) fn validate_payload_crc_streaming(
    file: &mut File,
    len: u64,
    expected: u32,
) -> Result<()> {
    let mut remaining = len;
    let mut buffer = [0u8; 64 * 1024];
    let mut hasher = crc32fast::Hasher::new();
    while remaining > 0 {
        let read_len = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("CRC buffer length fits usize");
        file.read_exact(&mut buffer[..read_len])
            .context("read framed-log payload for streaming CRC")?;
        hasher.update(&buffer[..read_len]);
        remaining -= read_len as u64;
    }
    let actual = hasher.finalize();
    if actual != expected {
        bail!("framed-log payload CRC mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}
