use super::*;

pub(super) enum LogWritePlan {
    Unchanged(Option<LogLayout>),
    Append {
        previous: LogLayout,
        next: LogLayout,
        bytes: Vec<u8>,
    },
    Rewrite {
        next: Option<LogLayout>,
        bytes: Vec<u8>,
    },
}

impl LogWritePlan {
    pub(super) fn layout(&self) -> Option<LogLayout> {
        match self {
            Self::Unchanged(layout) => *layout,
            Self::Append { next, .. } => Some(*next),
            Self::Rewrite { next, .. } => *next,
        }
    }
}

pub(super) fn plan_log_write(
    state: &PersistedStateRef<'_>,
    previous: Option<LogLayout>,
) -> LogWritePlan {
    let Some(first) = state.log.first() else {
        return if previous.is_some() {
            LogWritePlan::Rewrite {
                next: None,
                bytes: Vec::new(),
            }
        } else {
            LogWritePlan::Unchanged(None)
        };
    };
    let last = state.log.last().expect("non-empty Raft log has a tail");
    let last_digest = entry_digest(last);
    if let Some(previous) = previous {
        if previous.entry_count == state.log.len() as u64
            && previous.first_index == first.index.get()
            && previous.last_index == last.index.get()
            && previous.last_term == last.term.get()
            && previous.last_entry_digest == last_digest
        {
            return LogWritePlan::Unchanged(Some(previous));
        }
        if state.log.len() as u64 > previous.entry_count
            && previous.first_index == first.index.get()
            && previous.entry_count > 0
        {
            let prior_tail = &state.log[previous.entry_count as usize - 1];
            if prior_tail.index.get() == previous.last_index
                && prior_tail.term.get() == previous.last_term
                && entry_digest(prior_tail) == previous.last_entry_digest
            {
                let bytes = encode_log_entries(&state.log[previous.entry_count as usize..], false);
                let next = LogLayout {
                    generation: previous.generation,
                    byte_len: previous.byte_len.saturating_add(bytes.len() as u64),
                    entry_count: state.log.len() as u64,
                    first_index: first.index.get(),
                    last_index: last.index.get(),
                    last_term: last.term.get(),
                    last_entry_digest: last_digest,
                };
                return LogWritePlan::Append {
                    previous,
                    next,
                    bytes,
                };
            }
        }
    }

    let bytes = encode_log_entries(state.log, true);
    let mut generation = Sha256::new();
    generation.update(b"raft-log-generation-v1");
    generation.update(
        previous
            .map(|layout| layout.generation)
            .unwrap_or([0_u8; 32]),
    );
    generation.update(state.snapshot_index.get().to_le_bytes());
    generation.update(state.snapshot_term.get().to_le_bytes());
    generation.update(&bytes);
    let next = LogLayout {
        generation: generation.finalize().into(),
        byte_len: bytes.len() as u64,
        entry_count: state.log.len() as u64,
        first_index: first.index.get(),
        last_index: last.index.get(),
        last_term: last.term.get(),
        last_entry_digest: last_digest,
    };
    LogWritePlan::Rewrite {
        next: Some(next),
        bytes,
    }
}
