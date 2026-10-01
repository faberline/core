//! `transport-h2c` — shared HTTP/2 cleartext (h2c) client helpers for the ecosystem.
//!
//! Several components (loom → keep/relay, lumen's relay WAL, relay's raft peer
//! transport) talk to each other over **h2c** (HTTP/2 over cleartext, via
//! prior-knowledge — no TLS, no ALPN). Each used to hand-roll
//! `reqwest::Client::builder().http2_prior_knowledge().build()`. This crate
//! centralizes that, plus the connection-pool sizing that actually makes h2c
//! fast.
//!
//! ## Why a pool — the connection-count heuristic
//!
//! A single h2 connection multiplexes every stream, but all of its framing /
//! HPACK work serializes through one read/write task, so throughput bottlenecks
//! on **one core**. Spreading streams over a *few* connections recovers
//! multi-core throughput while keeping the connection count far below
//! HTTP/1.1's one-per-concurrent-request. Empirically (see
//! `examples/conn_sweep.rs`) throughput saturates around `ln(concurrency)`
//! connections, after which extra connections only add sockets. So:
//!
//! ```text
//! connections = clamp(ceil(ln(concurrency)), 1, cpu_parallelism)
//! ```
//!
//! `ln` grows so slowly it self-caps below the core count for any realistic
//! concurrency (`ln(22026) ≈ 10`), which is exactly why it tracks the knee
//! without ever over-provisioning.

mod client;
// The frame-level connection manager — built on the low-level `h2` crate so it
// can see GOAWAY / ping / flow-control and actively manage connections, where
// `H2cPool` (below) is the simpler reqwest-level round-robin option.
mod conn;
mod error;
mod http_method;
pub mod llm;
mod manager;
mod pool;
mod sizing;

pub use client::{h2c_client, h2c_client_with};
pub use error::{H2cError, Result};
pub use manager::{H2cManager, ManagerConfig, ManagerStats};
pub use pool::H2cPool;
pub use sizing::{cpu_parallelism, recommended_h2c_connections, recommended_h2c_connections_for};

// Per-connection server transport. Listener admission and lifecycle stay in
// `server-http`/`server-tcp`; client-only consumers do not link this feature.
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "server")]
pub use server::{
    serve_connection, serve_connection_with_drain, serve_connection_with_lifecycle,
    serve_connection_with_options, serve_io_with_drain, serve_io_with_lifecycle, ConnectionError,
    ConnectionOptions, ConnectionProtocol, ConnectionReport, ConnectionTerminal,
};
