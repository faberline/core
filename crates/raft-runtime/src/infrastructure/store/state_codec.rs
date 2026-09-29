use super::*;

impl RaftStore {
    pub(super) fn decode_persisted_state(&self, bytes: &[u8]) -> io::Result<PersistedState> {
        if bytes.starts_with(MAGIC_V4) {
            let mut r = CursorReader(&bytes[MAGIC_V4.len()..]);
            let term = r.read_u64()?;
            let voted_for = match r.read_u8()? {
                0 => None,
                1 => Some(r.read_u64()?),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid voted_for tag",
                    ))
                }
            };
            let commit_index = r.read_u64()?;
            let snapshot_index = r.read_u64()?;
            let snapshot_term = r.read_u64()?;
            let snapshot_len = r.read_u64()? as usize;
            let snapshot_digest = r.read_bytes(32)?;
            let snapshot = self.read_snapshot_artifact(
                snapshot_index,
                snapshot_term,
                snapshot_len,
                &snapshot_digest,
            )?;
            let conf = match r.read_u8()? {
                0 => None,
                1 => {
                    let (conf, consumed) = ConfState::decode_with_len(r.0).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "invalid conf state")
                    })?;
                    r.0 = &r.0[consumed..];
                    Some(conf)
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid conf tag",
                    ))
                }
            };
            let layout = match r.read_u8()? {
                0 => None,
                1 => Some(LogLayout {
                    generation: r.read_bytes(32)?.try_into().unwrap(),
                    byte_len: r.read_u64()?,
                    entry_count: r.read_u64()?,
                    first_index: r.read_u64()?,
                    last_index: r.read_u64()?,
                    last_term: r.read_u64()?,
                    last_entry_digest: r.read_bytes(32)?.try_into().unwrap(),
                }),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid Raft log artifact tag",
                    ))
                }
            };
            if !r.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailing bytes after persisted state",
                ));
            }
            let (log, commands) = match layout {
                Some(layout) => self.read_log_artifact(layout)?,
                None => (Vec::new(), BTreeMap::new()),
            };
            let mut cache = self.cache.lock().expect("raft store cache poisoned");
            cache.log = layout.map(|layout| PublishedLog { layout, commands });
            cache.commit_index = commit_index;
            Ok(PersistedState {
                term,
                voted_for,
                log,
                commit_index,
                snapshot_index,
                snapshot_term,
                snapshot,
                conf,
            })
        } else if bytes.starts_with(MAGIC_V3) {
            let mut r = CursorReader(&bytes[MAGIC_V3.len()..]);
            let term = r.read_u64()?;
            let has_voted_for = r.read_u8()?;
            let voted_for = match has_voted_for {
                0 => None,
                1 => Some(r.read_u64()?),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid voted_for tag",
                    ))
                }
            };
            let commit_index = r.read_u64()?;
            let snapshot_index = r.read_u64()?;
            let snapshot_term = r.read_u64()?;
            let snapshot_len = r.read_u64()? as usize;
            let snapshot_digest = r.read_bytes(32)?;

            let snapshot = self.read_snapshot_artifact(
                snapshot_index,
                snapshot_term,
                snapshot_len,
                &snapshot_digest,
            )?;

            let has_conf = r.read_u8()?;
            let conf = match has_conf {
                0 => None,
                1 => {
                    let (conf, consumed) = ConfState::decode_with_len(r.0).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "invalid conf state")
                    })?;
                    r.0 = &r.0[consumed..];
                    Some(conf)
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid conf tag",
                    ))
                }
            };

            let log_len = r.read_u64()? as usize;
            let mut log = Vec::with_capacity(log_len.min(100_000));
            for _ in 0..log_len {
                let entry_term = r.read_u64()?;
                let entry_index = r.read_u64()?;
                let kind_tag = r.read_u8()?;
                let kind = match kind_tag {
                    0 => EntryKind::Command,
                    1 => EntryKind::Config,
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "invalid entry kind tag",
                        ))
                    }
                };
                let command_len = r.read_u64()? as usize;
                let command = r.read_bytes(command_len)?;
                if kind == EntryKind::Config && ConfState::decode(&command).is_none() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid conf state",
                    ));
                }
                log.push(RaftEntry {
                    term: entry_term,
                    index: entry_index,
                    command,
                    kind,
                });
            }
            if !r.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailing bytes after persisted state",
                ));
            }
            Ok(PersistedState {
                term,
                voted_for,
                log,
                commit_index,
                snapshot_index,
                snapshot_term,
                snapshot,
                conf,
            })
        } else if bytes.starts_with(MAGIC_V2) {
            let mut r = CursorReader(&bytes[MAGIC_V2.len()..]);
            let term = r.read_u64()?;
            let has_voted_for = r.read_u8()?;
            let voted_for = match has_voted_for {
                0 => None,
                1 => Some(r.read_u64()?),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid voted_for tag",
                    ))
                }
            };
            let commit_index = r.read_u64()?;
            let snapshot_index = r.read_u64()?;
            let snapshot_term = r.read_u64()?;
            let snapshot_len = r.read_u64()? as usize;
            let snapshot_digest = r.read_bytes(32)?;

            let snapshot = self.read_snapshot_artifact(
                snapshot_index,
                snapshot_term,
                snapshot_len,
                &snapshot_digest,
            )?;

            let log_len = r.read_u64()? as usize;
            let mut log = Vec::with_capacity(log_len.min(100_000));
            for _ in 0..log_len {
                let entry_term = r.read_u64()?;
                let entry_index = r.read_u64()?;
                let command_len = r.read_u64()? as usize;
                let command = r.read_bytes(command_len)?;
                log.push(RaftEntry {
                    term: entry_term,
                    index: entry_index,
                    command,
                    kind: EntryKind::Command,
                });
            }
            if !r.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailing bytes after persisted state",
                ));
            }
            Ok(PersistedState {
                term,
                voted_for,
                log,
                commit_index,
                snapshot_index,
                snapshot_term,
                snapshot,
                conf: None,
            })
        } else if bytes.starts_with(MAGIC_V1) {
            let mut r = CursorReader(&bytes[MAGIC_V1.len()..]);
            let term = r.read_u64()?;
            let has_voted_for = r.read_u8()?;
            let voted_for = match has_voted_for {
                0 => None,
                1 => Some(r.read_u64()?),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid voted_for tag",
                    ))
                }
            };
            let commit_index = r.read_u64()?;
            let snapshot_index = r.read_u64()?;
            let snapshot_term = r.read_u64()?;
            let snapshot_len = r.read_u64()? as usize;
            let snapshot = r.read_bytes(snapshot_len)?;
            let log_len = r.read_u64()? as usize;
            let mut log = Vec::with_capacity(log_len.min(100_000));
            for _ in 0..log_len {
                let entry_term = r.read_u64()?;
                let entry_index = r.read_u64()?;
                let command_len = r.read_u64()? as usize;
                let command = r.read_bytes(command_len)?;
                log.push(RaftEntry {
                    term: entry_term,
                    index: entry_index,
                    command,
                    kind: EntryKind::Command,
                });
            }
            if !r.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailing bytes after persisted state",
                ));
            }
            Ok(PersistedState {
                term,
                voted_for,
                log,
                commit_index,
                snapshot_index,
                snapshot_term,
                snapshot,
                conf: None,
            })
        } else if let Some(&first_non_ws) = bytes.iter().find(|&&b| !b.is_ascii_whitespace()) {
            if first_non_ws == b'{' {
                serde_json::from_slice(bytes)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unrecognised durable state format marker",
                ))
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "empty durable state file",
            ))
        }
    }
}

pub(super) fn encode_persisted_state_v4(
    state: &PersistedStateRef<'_>,
    snapshot_digest: &[u8; 32],
    log: Option<LogLayout>,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(MAGIC_V4);
    bytes.extend_from_slice(&state.term.to_le_bytes());
    match state.voted_for {
        Some(node_id) => {
            bytes.push(1);
            bytes.extend_from_slice(&node_id.to_le_bytes());
        }
        None => bytes.push(0),
    }
    bytes.extend_from_slice(&state.commit_index.to_le_bytes());
    bytes.extend_from_slice(&state.snapshot_index.to_le_bytes());
    bytes.extend_from_slice(&state.snapshot_term.to_le_bytes());
    bytes.extend_from_slice(&(state.snapshot.len() as u64).to_le_bytes());
    bytes.extend_from_slice(snapshot_digest);
    match state.conf {
        Some(conf) => {
            bytes.push(1);
            bytes.extend_from_slice(&ConfState::encode(conf));
        }
        None => bytes.push(0),
    }
    match log {
        Some(log) => {
            bytes.push(1);
            bytes.extend_from_slice(&log.generation);
            bytes.extend_from_slice(&log.byte_len.to_le_bytes());
            bytes.extend_from_slice(&log.entry_count.to_le_bytes());
            bytes.extend_from_slice(&log.first_index.to_le_bytes());
            bytes.extend_from_slice(&log.last_index.to_le_bytes());
            bytes.extend_from_slice(&log.last_term.to_le_bytes());
            bytes.extend_from_slice(&log.last_entry_digest);
        }
        None => bytes.push(0),
    }
    bytes
}
