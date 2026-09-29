use thiserror::Error;

use crate::decode::{decode_write_request, DecodeError, ValidatedWrite};

/// A product hook that converts a validated request to domain data.
pub trait RemoteWriteConsumer {
    type Output;
    type Error;

    fn consume(&self, write: ValidatedWrite) -> Result<Self::Output, Self::Error>;
}

pub fn consume_write<C: RemoteWriteConsumer>(
    body: &[u8],
    consumer: &C,
) -> Result<C::Output, ConsumeError<C::Error>> {
    let write = decode_write_request(body).map_err(ConsumeError::Decode)?;
    consumer.consume(write).map_err(ConsumeError::Consumer)
}

#[derive(Debug, Error)]
pub enum ConsumeError<E> {
    #[error(transparent)]
    Decode(DecodeError),
    #[error("remote write consumer rejected the request")]
    Consumer(E),
}
