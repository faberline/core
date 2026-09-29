//! Prometheus Remote Write 1.0 transport and validation.
//!
//! This crate owns the wire contract. Products own the conversion from a
//! validated write request to their domain records.

mod consumer;
mod decode;
mod headers;
pub mod proto;

pub use consumer::{consume_write, ConsumeError, RemoteWriteConsumer};
pub use decode::{
    decode_snappy, decode_write_request, encode_snappy, DecodeError, ValidatedWrite,
    PROMETHEUS_STALE_NAN_BITS,
};
pub use headers::{validate_headers, HeaderError};
