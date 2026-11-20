//! Opt-in request correlation: the `x-request-id` header.
//!
//! [`request_id_middleware`] reads the `x-request-id` request header and keeps
//! it when it is visible ASCII and at most [`MAX_REQUEST_ID_BYTES`] bytes.
//! Otherwise (missing, empty, too long, not visible ASCII) it makes a new id.
//! The id is stored in the request extensions as a [`RequestId`], written back
//! to the request header (so a proxy behind this service forwards the same
//! id), and returned in the `x-request-id` response header. The handler runs
//! inside a tracing span named `request_id` with the field `request_id`, so
//! every log event of the handler carries the id.
//!
//! The id is for correlation only. It is not secret and not an authentication
//! value.
//!
//! [`MAX_REQUEST_ID_BYTES`]: service_observability::MAX_REQUEST_ID_BYTES

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use service_observability::MAX_REQUEST_ID_BYTES;
use tracing::Instrument as _;

/// The header that carries the request id, in lowercase.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// The id of one request, as stored in the request extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestId(String);

impl RequestId {
    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Keep or make the request id, store it in the request, and return it in the
/// `x-request-id` response header. See the module docs.
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| is_valid_request_id(value))
        .map(str::to_string)
        .unwrap_or_else(new_request_id);

    request.extensions_mut().insert(RequestId(id.clone()));
    if let Ok(value) = HeaderValue::from_str(&id) {
        request
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }

    let span = tracing::info_span!("request_id", request_id = %id);
    let mut response = next.run(request).instrument(span).await;
    if let Ok(value) = HeaderValue::from_str(&id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    response
}

/// `true` when the value is not empty, is at most [`MAX_REQUEST_ID_BYTES`]
/// bytes, and every byte is visible ASCII.
fn is_valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_BYTES
        && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A new id: 32 lowercase hex characters. No new dependency: each call hashes
/// the counter with a randomly seeded hasher.
fn new_request_id() -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let a = hash_lane(n, 0);
    let b = hash_lane(n, 1);
    format!("{a:016x}{b:016x}")
}

fn hash_lane(n: u64, lane: u8) -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(n);
    hasher.write_u8(lane);
    hasher.finish()
}
