//! Lock-free Prometheus metric primitives + text-format encoder.
//!
//! Every service in the kit needs the same three shapes — a monotonic
//! counter, a point-in-time gauge, and a `_sum`/`_count` latency
//! observation pair — rendered as Prometheus text format (0.0.4
//! compatible: `# HELP`/`# TYPE` lines followed by the sample). This
//! crate holds only those primitives: no registry side-table, no
//! macros, no dependencies. Callers own their metric structs (typically
//! one field per metric, as plain `Counter`/`Gauge`/`Latency` values)
//! and hand a slice of [`Sample`]s to [`render`] to produce an unlabeled
//! scrape body, or [`SampleGroup`]s to [`render_labeled`] when one HELP/TYPE
//! declaration owns multiple labeled rows.
//!
//! Lifted from lumen's `src/metrics.rs` (#974): lumen's `Metrics`
//! reimplements on top of these primitives with byte-identical
//! `render()` output; keep/relay/loom adoption is a future step.

mod histogram;
mod instrument;
mod render;

pub use histogram::{Bucket, Histogram};
pub use instrument::{Counter, Gauge, Latency};
pub use render::{
    escape_label_value, render, render_labeled, Label, LabeledSample, Sample, SampleGroup,
};
