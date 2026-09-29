use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

/// Standard INFO-level request tracing layer for HTTP runtimes.
pub fn trace_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>, DefaultMakeSpan> {
    TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
}
