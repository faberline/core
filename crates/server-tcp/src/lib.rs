//! Shared TCP accept/runtime layer.
//! @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#logic
//!
//! Protocol crates should implement [`TcpHandler`] and let this crate own the
//! listener loop, budget admission, task supervision, and drain behavior. HTTP
//! sits above this layer in `server-http`; raw protocol products such as a
//! Postgres pooler can use it directly.

mod config;
mod connection;
mod handler;
mod report;
mod serve;

pub use config::{TcpServerConfig, TcpSocketOptions};
pub use connection::{ConnectionContext, TcpConnectionResult, TcpConnectionTerminal};
pub use handler::TcpHandler;
pub use report::TcpServerReport;
pub use serve::{bind, serve, serve_arc, serve_with_report};
