//! HTTP method classification shared by the client manager and the server
//! transport.

use http::Method;

/// Whether `method` is safe in the RFC 9110 sense: GET, HEAD, OPTIONS and
/// TRACE. The manager retries only safe requests after losing a connection,
/// and the server counts an interrupted request that is not safe as
/// ambiguous.
pub(crate) fn is_safe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}
