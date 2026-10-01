//! The collector run loop and the ports a product implements: record
//! sources, decoders and batch sinks, plus runtime configuration and the
//! run report.

mod collect;
mod config;
mod report;
mod sink;
mod source;

pub use collect::{run_collector, run_collector_with_delivery_mode};
pub use config::{ConfigError, DeliveryRetryMode, RetryPolicy, RuntimeConfig};
pub use report::RunReport;
pub use sink::BatchSink;
pub use source::{CollectorSource, RecordDecoder};
