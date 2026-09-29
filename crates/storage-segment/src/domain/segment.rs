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

/// Durable local segment boundary. A product supplies its descriptor type.
pub trait SegmentStore<Record>: Send + Sync {
    type Descriptor: Clone + Send + Sync;

    fn write_immutable(&self, partition: &str, records: &[Record]) -> Result<Self::Descriptor>;
    fn read(&self, descriptor: &Self::Descriptor) -> Result<Vec<Record>>;
}
