use super::segment_error::Result;

/// Product codec for records stored in one immutable segment.
pub trait RecordCodec<Record>: Send + Sync {
    fn encode(&self, records: &[Record]) -> Result<Vec<u8>>;
    fn decode(&self, bytes: &[u8]) -> Result<Vec<Record>>;
}

/// Product policy that selects a stable partition for one record.
pub trait Partitioner<Record>: Send + Sync {
    fn partition(&self, record: &Record) -> Result<String>;
}
