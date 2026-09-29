use thiserror::Error;

use crate::signal::OtlpSignal;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("unsupported OTLP content-type `{media_type}`")]
    UnsupportedMediaType { media_type: String },
    #[error("unsupported OTLP content-encoding `{encoding}`")]
    UnsupportedContentEncoding { encoding: String },
    #[error("OTLP decoded body exceeds {maximum_bytes} bytes")]
    DecodedBodyTooLarge { maximum_bytes: usize },
    #[error("decode OTLP {signal:?} JSON: {message}")]
    InvalidJson { signal: OtlpSignal, message: String },
    #[error("decode OTLP {signal:?} protobuf: {message}")]
    InvalidProtobuf { signal: OtlpSignal, message: String },
    #[error("decode OTLP gzip body: {message}")]
    InvalidGzip { message: String },
    #[error("encode OTLP response: {message}")]
    Encode { message: String },
    #[error("OTLP consumer failed: {message}")]
    Consumer { message: String },
}

pub type Result<T> = std::result::Result<T, TransportError>;
