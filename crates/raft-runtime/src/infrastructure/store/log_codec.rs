use super::*;

pub(super) fn encode_log_entries(entries: &[RaftEntry], include_magic: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if include_magic {
        bytes.extend_from_slice(LOG_MAGIC_V1);
    }
    for entry in entries {
        let payload = encode_log_entry(entry);
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        bytes.extend_from_slice(&payload);
    }
    bytes
}

fn encode_log_entry(entry: &RaftEntry) -> Vec<u8> {
    let mut payload = Vec::with_capacity(25 + entry.command.len());
    payload.extend_from_slice(&entry.term.to_le_bytes());
    payload.extend_from_slice(&entry.index.to_le_bytes());
    payload.push(match entry.kind {
        EntryKind::Command => 0,
        EntryKind::Config => 1,
    });
    payload.extend_from_slice(&(entry.command.len() as u64).to_le_bytes());
    payload.extend_from_slice(&entry.command);
    payload
}

pub(super) fn entry_digest(entry: &RaftEntry) -> [u8; 32] {
    Sha256::digest(encode_log_entry(entry)).into()
}

pub(super) fn decode_log_artifact(bytes: &[u8], layout: LogLayout) -> io::Result<Vec<RaftEntry>> {
    if !bytes.starts_with(LOG_MAGIC_V1) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Raft log artifact marker",
        ));
    }
    let mut reader = CursorReader(&bytes[LOG_MAGIC_V1.len()..]);
    let mut log = Vec::with_capacity((layout.entry_count as usize).min(100_000));
    for _ in 0..layout.entry_count {
        let payload_len = usize::try_from(reader.read_u64()?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft log frame length overflow")
        })?;
        let expected_crc = reader.read_u32()?;
        let payload = reader.read_bytes(payload_len)?;
        if crc32fast::hash(&payload) != expected_crc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Raft log frame checksum mismatch",
            ));
        }
        let mut entry = CursorReader(&payload);
        let term = entry.read_u64()?;
        let index = entry.read_u64()?;
        let kind = match entry.read_u8()? {
            0 => EntryKind::Command,
            1 => EntryKind::Config,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid Raft log entry kind",
                ))
            }
        };
        let command_len = usize::try_from(entry.read_u64()?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft command length overflow")
        })?;
        let command = entry.read_bytes(command_len)?;
        if !entry.is_empty() || (kind == EntryKind::Config && ConfState::decode(&command).is_none())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Raft log entry payload",
            ));
        }
        log.push(RaftEntry {
            term,
            index,
            command,
            kind,
        });
    }
    if !reader.is_empty()
        || log.first().map(|entry| entry.index) != Some(layout.first_index)
        || log.last().map(|entry| entry.index) != Some(layout.last_index)
        || log.last().map(|entry| entry.term) != Some(layout.last_term)
        || log.last().map(entry_digest) != Some(layout.last_entry_digest)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Raft log artifact disagrees with hard state",
        ));
    }
    Ok(log)
}
