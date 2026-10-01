use std::fs::File;
use std::io::{BufWriter, Write};

use anyhow::{bail, Context, Result};

use super::codec::{HEADER_LEN, MAX_FRAME_PAYLOAD_BYTES};
use super::observer::FramedLogTrimObserver;

fn trim_read_exact_at(file: &File, mut bytes: &mut [u8], mut offset: u64) -> Result<()> {
    while !bytes.is_empty() {
        #[cfg(unix)]
        let read = {
            use std::os::unix::fs::FileExt;
            file.read_at(bytes, offset)
        };
        #[cfg(not(unix))]
        let read: std::io::Result<usize> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "staged log trim requires positional reads",
        ));
        match read {
            Ok(0) => bail!("trim source ended before captured EOF"),
            Ok(count) => {
                offset += count as u64;
                bytes = &mut bytes[count..];
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("read pinned trim source"),
        }
    }
    Ok(())
}

/// Read a private, writer-captured frame interval. Unlike torn-tail recovery,
/// trim refuses every incomplete frame. The temp remains unpublished while
/// streaming, so a CRC failure discards the plan, including partial output.
pub(super) fn stream_trim_range(
    source: &File,
    mut offset: u64,
    end: u64,
    through: u64,
    temp: &mut BufWriter<File>,
    observer: Option<&dyn FramedLogTrimObserver>,
) -> Result<bool> {
    if offset > end {
        bail!("trim frame interval is reversed");
    }
    let mut buffer = vec![0u8; 64 * 1024];
    let mut added = false;
    while offset < end {
        let frame_start = offset;
        if end - offset < HEADER_LEN as u64 {
            bail!("trim source contains a torn frame header");
        }
        let mut header = [0u8; HEADER_LEN];
        trim_read_exact_at(source, &mut header, offset)?;
        let seq = u64::from_le_bytes(header[0..8].try_into().unwrap());
        let len = u32::from_le_bytes(header[8..12].try_into().unwrap()) as u64;
        let expected_crc = u32::from_le_bytes(header[12..16].try_into().unwrap());
        offset += HEADER_LEN as u64;
        if len > end - offset {
            if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
                bail!("oversized legacy log frame at byte {frame_start} is incomplete; refusing destructive recovery");
            }
            bail!("trim source contains a torn frame payload");
        }
        let keep = seq > through;
        if keep {
            temp.write_all(&header).context("write trim frame header")?;
            added = true;
        }
        let mut crc = crc32fast::Hasher::new();
        let mut remaining = len;
        while remaining != 0 {
            let count = remaining.min(buffer.len() as u64) as usize;
            trim_read_exact_at(source, &mut buffer[..count], offset)?;
            crc.update(&buffer[..count]);
            if keep {
                temp.write_all(&buffer[..count])
                    .context("write trim frame payload")?;
            }
            offset += count as u64;
            remaining -= count as u64;
        }
        if crc.finalize() != expected_crc {
            if len > MAX_FRAME_PAYLOAD_BYTES as u64 {
                bail!("oversized legacy log frame at byte {frame_start} failed validation; refusing destructive recovery");
            }
            bail!("trim source frame CRC mismatch");
        }
        if !keep {
            if let Some(observer) = observer {
                observer.covered_frame(through, seq);
            }
        }
    }
    Ok(added)
}
