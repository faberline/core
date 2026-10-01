//! The h2c and TLS serve entry points, W3C request trace context, and the
//! access-logging trace layer.

mod access_log;
mod serve;
mod trace_context;

pub use access_log::{trace_layer, AccessLogOnRequest, AccessLogOnResponse, CorrelatingMakeSpan};
pub use serve::{serve, serve_tls, serve_with_lifecycle, PropagatingMakeSpan};
pub use trace_context::{request_trace_context, RequestTraceContext};
