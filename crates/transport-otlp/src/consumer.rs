use async_trait::async_trait;

use crate::codec::decode_payload;
use crate::error::Result;
use crate::media::OtlpMediaType;
use crate::payload::{DecodedPayload, PartialSuccess};
use crate::signal::OtlpSignal;

#[async_trait]
pub trait OtlpConsumer: Send + Sync {
    async fn consume(&self, project: &str, payload: DecodedPayload) -> Result<PartialSuccess>;
}

pub async fn dispatch<C>(
    consumer: &C,
    project: &str,
    signal: OtlpSignal,
    media_type: OtlpMediaType,
    body: &[u8],
) -> Result<PartialSuccess>
where
    C: OtlpConsumer + ?Sized,
{
    let payload = decode_payload(signal, media_type, body)?;
    consumer.consume(project, payload).await
}
