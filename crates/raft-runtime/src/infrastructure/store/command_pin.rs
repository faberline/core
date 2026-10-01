use super::*;

impl CommittedCommandLease {
    /// The exact command payload from the pinned durable frame.
    pub fn command(&self) -> &[u8] {
        &self.map[self.command_start..self.command_end]
    }
}

impl CommittedCommandPin {
    /// Map and validate the pinned artifact outside the host node mutex.
    pub fn map(self) -> io::Result<CommittedCommandLease> {
        let file = OpenOptions::new().read(true).open(&self.generation.path)?;
        let actual_len = file.metadata()?.len();
        if actual_len < self.generation.byte_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pinned Raft log artifact is truncated",
            ));
        }
        let mapped_len = usize::try_from(self.generation.byte_len).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Raft log artifact exceeds address space",
            )
        })?;
        let map =
            unsafe { MmapOptions::new().len(mapped_len).map(&file) }.map_err(io::Error::other)?;
        validate_pinned_frame(&map, self.frame)?;
        let command_start = usize::try_from(self.frame.command_offset).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft command offset overflow")
        })?;
        let command_len = usize::try_from(self.frame.command_len).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft command length overflow")
        })?;
        let command_end = command_start.checked_add(command_len).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Raft command range overflow")
        })?;
        if command_end > map.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pinned Raft command is truncated",
            ));
        }
        Ok(CommittedCommandLease {
            map,
            command_start,
            command_end,
            _generation: self.generation,
        })
    }
}

impl Drop for GenerationPin {
    fn drop(&mut self) {
        let mut pins = self
            .pins
            .lock()
            .expect("pinned log generations mutex poisoned");
        let count = pins
            .get_mut(&self.generation)
            .expect("generation pin missing from registry");
        *count -= 1;
        if *count == 0 {
            pins.remove(&self.generation);
        }
    }
}

impl RaftStore {
    /// Pin the published durable generation for exactly one committed command.
    ///
    /// This copies only frame metadata while the cache lock is held. The file
    /// is opened and mapped later by [`CommittedCommandPin::map`].
    pub fn pin_committed_command(
        &self,
        index: Index,
        term: Term,
    ) -> io::Result<CommittedCommandPin> {
        let cache = self.cache.lock().expect("raft store cache poisoned");
        if index > cache.commit_index {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "requested Raft command is not committed",
            ));
        }
        let published = cache.log.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "no published Raft log artifact",
            )
        })?;
        let frame = *published.commands.get(&(index, term)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "requested Raft command identity is not published",
            )
        })?;
        if frame.kind != EntryKind::Command {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "requested Raft identity is not a command",
            ));
        }
        let generation = published.layout.generation;
        let mut pins = self
            .pinned_log_generations
            .lock()
            .expect("pinned log generations mutex poisoned");
        *pins.entry(generation).or_default() += 1;
        Ok(CommittedCommandPin {
            generation: GenerationPin {
                generation,
                path: self.log_artifact_path(&generation),
                byte_len: published.layout.byte_len,
                pins: Arc::clone(&self.pinned_log_generations),
            },
            frame,
        })
    }
}

fn validate_pinned_frame(map: &[u8], frame: CommandFrame) -> io::Result<()> {
    if !map.starts_with(LOG_MAGIC_V1) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Raft log artifact marker",
        ));
    }
    let payload_start = usize::try_from(frame.payload_offset)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Raft payload offset overflow"))?;
    let payload_len = usize::try_from(frame.payload_len)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Raft payload length overflow"))?;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Raft payload range overflow"))?;
    let payload = map.get(payload_start..payload_end).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "pinned Raft frame is truncated")
    })?;
    let frame_start = payload_start.checked_sub(12).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Raft frame header offset underflow",
        )
    })?;
    let header = map.get(frame_start..payload_start).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "pinned Raft frame header is truncated",
        )
    })?;
    let encoded_len = u64::from_le_bytes(header[..8].try_into().unwrap());
    let encoded_crc = u32::from_le_bytes(header[8..].try_into().unwrap());
    if encoded_len != frame.payload_len || encoded_crc != frame.payload_crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "pinned Raft frame header disagrees with metadata",
        ));
    }
    if crc32fast::hash(payload) != frame.payload_crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "pinned Raft frame checksum mismatch",
        ));
    }
    let mut entry = CursorReader(payload);
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
    let command_len = entry.read_u64()?;
    let command_len_usize = usize::try_from(command_len)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Raft command length overflow"))?;
    if index != frame.index
        || term != frame.term
        || kind != frame.kind
        || command_len != frame.command_len
        || entry.0.len() != command_len_usize
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "pinned Raft frame identity disagrees with metadata",
        ));
    }
    let expected_command_offset = frame.payload_offset.checked_add(25).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Raft command offset overflow")
    })?;
    if frame.command_offset != expected_command_offset {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "pinned Raft command range disagrees with metadata",
        ));
    }
    Ok(())
}
