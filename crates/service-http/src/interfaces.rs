//! Entry surfaces: serving, probes, request tracing, the admission, body-limit
//! and Server-Timing middleware, request decoding, the error envelope, the
//! reverse proxy and the signal bridge.

mod admission_middleware;
mod body_limit;
mod content_decode;
mod error;
mod probes;
mod reverse_proxy;
mod server_timing;
mod signal;
mod transport;

pub use admission_middleware::{admission_middleware, AdmissionMiddleware};
pub use body_limit::{body_limit_layer, BodyLimitLayer, BodyLimitService};
pub use content_decode::{
    decode_request_body, ContentDecodeError, ContentDecodeErrorKind, ContentDecodeLimitError,
    ContentDecodeLimits,
};
pub use error::{
    retry_delay_from_detailed_error, ApiErr, DetailedErrorEnvelope, ErrorEnvelope,
    ProjectionMetadata,
};
pub use probes::{
    lifecycle_probe_routes, lifecycle_probe_routes_canonical_json, standard_probe_routes,
    standard_probe_routes_canonical_json,
};
pub use reverse_proxy::{reverse_proxy_router, ReverseProxyPolicy, ReverseProxySelectionError};
pub use server_timing::{server_timing_middleware, ServerTimingDisclosure, ServerTimingExt};
pub use signal::{
    run_signal_bridge, shutdown_on_signal, shutdown_with_drain, wait_shutdown_signal,
    LifecycleShutdownTrigger,
};
pub use transport::{
    request_trace_context, serve, serve_tls, serve_with_lifecycle, trace_layer, AccessLogOnRequest,
    AccessLogOnResponse, CorrelatingMakeSpan, PropagatingMakeSpan, RequestTraceContext,
};
