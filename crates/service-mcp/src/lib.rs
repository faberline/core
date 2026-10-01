//! Shared Model Context Protocol transport and security shell.

mod application;
mod config;
mod http;
mod stdio;

pub use application::McpApplication;
pub use config::HttpTransportConfig;
pub use http::streamable_http_router;
pub use stdio::serve_stdio;
