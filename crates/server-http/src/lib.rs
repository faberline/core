//! Shared HTTP runtime above `server-lifecycle` and `server-tcp`.
//! @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#logic
//!
//! This crate is intentionally below `service-http`: it provides HTTP serving
//! primitives for both production services and tool/dev servers. Service
//! archetype policy such as `/healthz`, `/readyz`, `/metrics`, OpenAPI, and
//! docs remains in `service-http`.

mod options;
mod serve;
pub mod tls;
mod trace;

pub use options::{H2cServerOptions, HttpServerOptions};
pub use serve::{serve_h2c, serve_h2c_with_lifecycle, serve_h2c_with_options, HttpServerReport};
pub use server_lifecycle as core;
pub use server_tcp as tcp;
pub use tls::{
    config_source, serve_tls, ServerConfigSource, TlsListenerMetrics, TlsListenerSnapshot,
    TlsServerOptions,
};
pub use trace::trace_layer;
