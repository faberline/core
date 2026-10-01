use std::sync::atomic::{AtomicBool, Ordering};
use std::thread_local;

use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::{MakeSpan, OnRequest, TraceLayer};

use super::trace_context::request_trace_context;

/// Request span maker that always records canonical correlation fields and,
/// when an OpenTelemetry layer is installed, attaches the same valid W3C
/// parent context to the exported span.
#[derive(Debug, Clone, Copy)]
pub struct CorrelatingMakeSpan;

impl<B> MakeSpan<B> for CorrelatingMakeSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        let context = request_trace_context(request.headers());
        let span = tracing::span!(
            tracing::Level::INFO,
            "request",
            method = %request.method(),
            uri = %request.uri(),
            version = ?request.version(),
            trace_id = %context.trace_id(),
            span_id = %context.span_id(),
            parent_span_id = tracing::field::Empty,
            trace_flags = %context.trace_flags(),
            subject = "anonymous",  // Default subject; overridden by auth middleware if authenticated
        );
        if let Some(parent_span_id) = context.parent_span_id() {
            span.record("parent_span_id", tracing::field::display(parent_span_id));
        }
        #[cfg(feature = "otlp")]
        {
            use opentelemetry::trace::TraceContextExt as _;
            use tracing_opentelemetry::OpenTelemetrySpanExt as _;
            let parent = service_observability::extract_trace_context(request.headers());
            if parent.span().span_context().is_valid() {
                span.set_parent(parent);
                let exported = span.context();
                let exported = exported.span();
                let exported = exported.span_context();
                if exported.is_valid() {
                    span.record("trace_id", tracing::field::display(exported.trace_id()));
                    span.record("span_id", tracing::field::display(exported.span_id()));
                    span.record(
                        "trace_flags",
                        tracing::field::display(format_args!(
                            "{:02x}",
                            exported.trace_flags().to_u8()
                        )),
                    );
                }
            }
        }
        span
    }
}

// Thread-local to track whether the current request is a probe request.
// Set by AccessLogOnRequest, read by AccessLogOnResponse.
thread_local! {
    static CURRENT_REQUEST_IS_PROBE: AtomicBool = const { AtomicBool::new(false) };
}

/// OnRequest handler that marks whether this is a probe path.
#[derive(Debug, Clone, Copy)]
pub struct AccessLogOnRequest;

impl<B> OnRequest<B> for AccessLogOnRequest {
    fn on_request(&mut self, request: &axum::http::Request<B>, _span: &tracing::Span) {
        let is_probe = matches!(
            request.uri().path(),
            "/healthz" | "/readyz" | "/metrics" | "/openapi.json" | "/docs"
        );
        CURRENT_REQUEST_IS_PROBE.with(|flag| flag.store(is_probe, Ordering::Relaxed));
    }
}

/// Custom HTTP response recorder that emits access log events.
/// Emits INFO-level events for regular requests, DEBUG-level for probe paths.
#[derive(Debug, Clone, Copy)]
pub struct AccessLogOnResponse;

impl<B> tower_http::trace::OnResponse<B> for AccessLogOnResponse {
    fn on_response(
        self,
        response: &axum::http::Response<B>,
        latency: std::time::Duration,
        _span: &tracing::Span,
    ) {
        let status = response.status().as_u16();
        let latency_ms = latency.as_millis() as u64;

        // Check if this is a probe path (set by AccessLogOnRequest)
        let is_probe = CURRENT_REQUEST_IS_PROBE.with(|flag| flag.load(Ordering::Relaxed));

        if is_probe {
            // Probe paths get DEBUG-level events
            tracing::debug!(
                target: "http.access",
                status = status,
                latency_ms = latency_ms,
                "access"
            );
        } else {
            // Regular requests get INFO-level events
            tracing::info!(
                target: "http.access",
                status = status,
                latency_ms = latency_ms,
                "access"
            );
        }
    }
}

/// The standard request-tracing layer: one INFO-level span per HTTP request.
///
/// Also emits per-request access log events at target "http.access":
/// - INFO level for non-probe requests, with `status` and `latency_ms` fields.
/// - DEBUG level for probe requests (/healthz, /readyz, /metrics, /openapi.json, /docs).
///
/// INFO so the default `info` `EnvFilter` keeps it, and so the spans the OTLP
/// layer (when wired) would export are produced. Attach it to the **outer**
/// router so it spans probe and data-plane requests alike:
///
/// ```ignore
/// let app = service_http::standard_probe_routes(readiness, metrics, openapi)
///     .merge(data_plane)
///     .layer(service_http::trace_layer())
///     .with_state(state);
/// ```
///
/// Returns the concrete `TraceLayer` so callers `.layer()` it directly. For a
/// different classifier/make-span, build `TraceLayer::new_for_http()` inline
/// instead.
pub fn trace_layer() -> TraceLayer<
    SharedClassifier<ServerErrorsAsFailures>,
    CorrelatingMakeSpan,
    AccessLogOnRequest,
    AccessLogOnResponse,
> {
    TraceLayer::new_for_http()
        .make_span_with(CorrelatingMakeSpan)
        .on_request(AccessLogOnRequest)
        .on_response(AccessLogOnResponse)
}
