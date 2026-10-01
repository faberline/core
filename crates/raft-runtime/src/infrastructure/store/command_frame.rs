use super::*;

pub(super) enum CommandFrameUpdate {
    Unchanged,
    Append(Vec<CommandFrame>),
    Rewrite(BTreeMap<(Index, Term), CommandFrame>),
    Clear,
}

pub(super) fn command_frame_update(plan: &LogWritePlan) -> io::Result<CommandFrameUpdate> {
    match plan {
        LogWritePlan::Unchanged(_) => Ok(CommandFrameUpdate::Unchanged),
        LogWritePlan::Append {
            previous,
            next,
            bytes,
        } => Ok(CommandFrameUpdate::Append(
            command_frames_from_encoded(
                bytes,
                next.entry_count
                    .checked_sub(previous.entry_count)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Raft append entry count underflow",
                        )
                    })?,
                previous.byte_len,
                false,
            )?
            .into_values()
            .collect(),
        )),
        LogWritePlan::Rewrite {
            next: Some(next),
            bytes,
        } => Ok(CommandFrameUpdate::Rewrite(command_frames_from_encoded(
            bytes,
            next.entry_count,
            LOG_MAGIC_V1.len() as u64,
            true,
        )?)),
        LogWritePlan::Rewrite { next: None, .. } => Ok(CommandFrameUpdate::Clear),
    }
}

pub(super) fn apply_command_frame_update(
    cache: &mut StoreCache,
    next_log: Option<LogLayout>,
    update: CommandFrameUpdate,
) {
    match update {
        CommandFrameUpdate::Unchanged => {
            debug_assert_eq!(cache.log.as_ref().map(|log| log.layout), next_log);
        }
        CommandFrameUpdate::Append(frames) => {
            let published = cache
                .log
                .as_mut()
                .expect("validated append has published Raft command metadata");
            for frame in frames {
                let previous = published
                    .commands
                    .insert((frame.index, frame.term), frame)
                    .is_some();
                debug_assert!(
                    !previous,
                    "validated append has distinct command identities"
                );
            }
            published.layout = next_log.expect("validated append has next Raft log layout");
        }
        CommandFrameUpdate::Rewrite(commands) => {
            cache.log = Some(PublishedLog {
                layout: next_log.expect("validated rewrite has next Raft log layout"),
                commands,
            });
        }
        CommandFrameUpdate::Clear => cache.log = None,
    }
}

pub(super) fn validate_command_frame_update(
    cache: &StoreCache,
    next_log: Option<LogLayout>,
    update: &CommandFrameUpdate,
) -> io::Result<()> {
    match update {
        CommandFrameUpdate::Unchanged => {
            if cache.log.as_ref().map(|log| log.layout) != next_log {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unchanged Raft log metadata disagrees with cache",
                ));
            }
        }
        CommandFrameUpdate::Append(frames) => {
            let published = cache.log.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "append has no published Raft command metadata",
                )
            })?;
            if next_log.is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "append has no next Raft log layout",
                ));
            }
            if frames
                .iter()
                .any(|frame| published.commands.contains_key(&(frame.index, frame.term)))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "duplicate Raft command identity in log",
                ));
            }
        }
        CommandFrameUpdate::Rewrite(_) if next_log.is_none() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "rewrite has no next Raft log layout",
            ));
        }
        CommandFrameUpdate::Rewrite(_) | CommandFrameUpdate::Clear => {}
    }
    Ok(())
}

pub(super) fn command_frames_from_artifact(
    bytes: &[u8],
    layout: LogLayout,
) -> io::Result<BTreeMap<(Index, Term), CommandFrame>> {
    command_frames_from_encoded(bytes, layout.entry_count, LOG_MAGIC_V1.len() as u64, true)
}

fn command_frames_from_encoded(
    bytes: &[u8],
    entry_count: u64,
    mut frame_offset: u64,
    include_magic: bool,
) -> io::Result<BTreeMap<(Index, Term), CommandFrame>> {
    let encoded = if include_magic {
        if !bytes.starts_with(LOG_MAGIC_V1) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Raft log artifact marker",
            ));
        }
        &bytes[LOG_MAGIC_V1.len()..]
    } else {
        bytes
    };
    if include_magic && frame_offset != LOG_MAGIC_V1.len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Raft log frame base offset",
        ));
    }
    let mut reader = CursorReader(encoded);
    let mut commands = BTreeMap::new();
    for _ in 0..entry_count {
        let payload_len = usize::try_from(reader.read_u64()?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft log frame length overflow")
        })?;
        let payload_crc = reader.read_u32()?;
        let payload = reader.read_slice(payload_len)?;
        let mut entry = CursorReader(&payload);
        let term = Term::new(entry.read_u64()?);
        let index = Index::new(entry.read_u64()?);
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
        let command_len = entry.read_u64()?;
        let command_len_usize = usize::try_from(command_len).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft command length overflow")
        })?;
        entry.skip_bytes(command_len_usize)?;
        if !entry.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Raft log entry payload",
            ));
        }
        let payload_len_u64 = payload_len as u64;
        let payload_offset = frame_offset.checked_add(12).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft frame offset overflow")
        })?;
        let command_offset = payload_offset.checked_add(25).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft command offset overflow")
        })?;
        let frame = CommandFrame {
            index,
            term,
            kind,
            payload_offset,
            payload_len: payload_len_u64,
            payload_crc,
            command_offset,
            command_len,
        };
        if commands.insert((index, term), frame).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate Raft command identity in log",
            ));
        }
        frame_offset = frame_offset
            .checked_add(12)
            .and_then(|offset| offset.checked_add(payload_len_u64))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Raft frame offset overflow")
            })?;
    }
    if !reader.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Raft log artifact disagrees with hard state",
        ));
    }
    Ok(commands)
}
