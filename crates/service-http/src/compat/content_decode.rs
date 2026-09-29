//! Bounded request decoding for identity and gzip content encodings.

pub use crate::interfaces::{
    decode_request_body, ContentDecodeError, ContentDecodeErrorKind, ContentDecodeLimitError,
    ContentDecodeLimits,
};
