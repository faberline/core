//! HTTP transport: the h2c serve loop + the standard request-tracing layer.
//!
//! [`serve`] composes [`server_http::serve_h2c`] (HTTP/1.1 + HTTP/2 cleartext on one port —
//! the in-cluster default `axum::serve` can't do) rather than re-implementing
//! the accept loop. [`trace_layer`] is the one INFO-level span-per-request layer
//! lumen/keep both attach; a service `.layer(...)`s it onto its router.

pub use crate::interfaces::{
    request_trace_context, serve, serve_tls, serve_with_lifecycle, trace_layer, AccessLogOnRequest,
    AccessLogOnResponse, CorrelatingMakeSpan, PropagatingMakeSpan, RequestTraceContext,
};
