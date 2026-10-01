# metrics-prometheus

## Brief

`metrics-prometheus` provides lock-free Prometheus metric primitives and a text
format encoder for service endpoints, including deterministic labeled metric
groups with Prometheus-safe label escaping.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Shared Prometheus Metric Primitives | - | counters, gauges, latency observations, and labeled/unlabeled encoders |

### Shared Prometheus Metric Primitives

Services can expose deterministic Prometheus text metrics through shared
lock-free primitives without service-specific encoders. Labeled groups emit one
HELP/TYPE declaration, preserve caller row order, sort labels
deterministically, and escape backslash, quote, and newline values.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `metrics_prometheus`.
- Gate — behavior: `cargo test -p metrics-prometheus` - metric primitive and
  encoder coverage
- Gate: `cargo test -p metrics-prometheus`
- Gate: `cargo test -p pgpool --lib`
- Source: `crates/metrics-prometheus/src/instrument.rs`,
  `crates/metrics-prometheus/src/histogram.rs`,
  `crates/metrics-prometheus/src/render.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| shared-prometheus-metric-primitives-contract | epic | - | `cargo test -p metrics-prometheus`; crates/metrics-prometheus/src/render.rs |
| labeled-sample-encoder-adoption | change | #1765 | `cargo test -p metrics-prometheus`; `cargo test -p pgpool --lib` |
