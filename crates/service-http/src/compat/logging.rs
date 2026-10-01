//! Compatibility adapter to protocol-neutral `service-observability`.

#[cfg(feature = "otlp")]
pub use crate::infrastructure::extract_trace_context;
pub use crate::infrastructure::{
    init_tracing, init_tracing_with_identity, tracing_mode, OtelFallback, TracingMode,
};
