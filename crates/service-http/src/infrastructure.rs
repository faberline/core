//! Adapters: the resolved HTTP service configuration and the tracing install
//! over service-observability.

mod config;
mod logging;

pub use config::{HttpConfig, LogFormat, ServiceIdentity};
#[cfg(feature = "otlp")]
pub use logging::extract_trace_context;
pub use logging::{
    init_tracing, init_tracing_with_identity, tracing_mode, OtelFallback, TracingMode,
};
