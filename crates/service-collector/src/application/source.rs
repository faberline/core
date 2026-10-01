use std::fmt;

use anyhow::Result;

use crate::domain::{
    CollectorRecord, CollectorRejection, CommitStats, ReadOutcome, SourceProgress,
};

pub trait CollectorSource {
    type Cursor: Clone;
    type Error: fmt::Display + Send + Sync + 'static;
    type Record: CollectorRecord<Cursor = Self::Cursor>;
    type Rejection: CollectorRejection<Cursor = Self::Cursor>;

    fn next_record(
        &mut self,
        max_bytes: usize,
    ) -> Result<ReadOutcome<Self::Record, Self::Rejection>, Self::Error>;
    fn commit(&mut self, cursors: &[Self::Cursor], stats: CommitStats) -> Result<(), Self::Error>;
    fn refresh(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn progress(&self) -> SourceProgress {
        SourceProgress::default()
    }
}

pub trait RecordDecoder<R> {
    type Item;
    type Rejection;

    fn decode(&self, record: R) -> Result<Self::Item, Self::Rejection>;
}
