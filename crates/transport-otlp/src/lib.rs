//! Official OTLP transport shell.
//!
//! Products implement [`OtlpConsumer`]. This crate owns media negotiation,
//! official protobuf decoding, bounded gzip, and partial-success encoding.

mod codec;
mod consumer;
mod error;
mod grpc;
mod media;
mod payload;
pub mod proto;
mod signal;

pub use codec::{decode_content_encoding, decode_payload, encode_response};
pub use consumer::{dispatch, OtlpConsumer};
pub use error::{Result, TransportError};
pub use grpc::{serve_grpc, GrpcProjectAuthorizer};
pub use media::OtlpMediaType;
pub use payload::{DecodedPayload, EncodedOtlpResponse, PartialSuccess};
pub use signal::OtlpSignal;
