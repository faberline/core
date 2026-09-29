//! Product-neutral collector runtime.
//!
//! A product provides sources, record decoding, and delivery. This crate owns
//! batching, retry, quarantine ordering, and checkpoint commit ordering.

mod application;
mod domain;
mod infrastructure;

pub use application::{
    run_collector, run_collector_with_delivery_mode, BatchSink, CollectorSource, ConfigError,
    DeliveryRetryMode, RecordDecoder, RetryPolicy, RunReport, RuntimeConfig,
};
pub use domain::{
    CollectorRecord, CollectorRejection, CommitStats, DeliveryFailure, DeliveryReceipt,
    QuarantineSink, ReadOutcome, SourceProgress,
};
pub use infrastructure::{
    append_jsonl, load_json_checkpoint, save_json_checkpoint, JsonlQuarantine,
};
