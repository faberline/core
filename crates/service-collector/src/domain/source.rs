#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommitStats {
    pub accepted: u64,
    pub duplicates: u64,
    pub rejected: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceProgress {
    pub start_offset: u64,
    pub final_offset: u64,
    pub lost_bytes: u64,
    pub lost_sources: u64,
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
