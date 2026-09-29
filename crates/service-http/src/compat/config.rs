//! HTTP-specific runtime knobs and observability-config projection.
//!
//! Each service binary already parses these (under its own `SERVICE_*`
//! prefix via clap `env =`); this is the resolved, prefix-agnostic struct the
//! shared HTTP scaffolding reads. Protocol-neutral logging/tracing types live
//! in `service-observability` and are re-exported here for compatibility.

pub use crate::infrastructure::{HttpConfig, LogFormat, ServiceIdentity};
