#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommitStats {
    pub accepted: u64,
    pub duplicates: u64,
    pub rejected: u64,
}

/// Where a source started and has reached, and what it reports lost.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceProgress {
    start_offset: SourceOffset,
    final_offset: SourceOffset,
    lost_bytes: u64,
    lost_sources: u64,
}

impl SourceProgress {
    pub fn new(
        start_offset: SourceOffset,
        final_offset: SourceOffset,
        lost_bytes: u64,
        lost_sources: u64,
    ) -> Self {
        Self {
            start_offset,
            final_offset,
            lost_bytes,
            lost_sources,
        }
    }

    pub fn start_offset(&self) -> SourceOffset {
        self.start_offset
    }

    pub fn final_offset(&self) -> SourceOffset {
        self.final_offset
    }

    pub fn lost_bytes(&self) -> u64 {
        self.lost_bytes
    }

    pub fn lost_sources(&self) -> u64 {
        self.lost_sources
    }
}

pub trait CollectorRecord {
    type Cursor: Clone;

    fn cursor(&self) -> &Self::Cursor;
}

pub trait CollectorRejection {
    type Cursor: Clone;
    type Entry;

    fn into_parts(self) -> (Self::Entry, Self::Cursor);
}

pub enum ReadOutcome<R, Q> {
    Record(R),
    Rejection(Q),
    Pending,
    Exhausted,
}
use super::SourceOffset;
