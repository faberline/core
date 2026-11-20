//! Opt-in per-operation request metrics.
//!
//! [`operation_metrics_middleware`] counts requests, counts errors (status 500
//! or higher) and sums latency for each operation and method. The operation is
//! the axum route template (`MatchedPath`), not the raw path, so the label set
//! stays small. [`OperationMetrics`] renders the series through
//! [`MetricsProvider`].
//!
//! With `Router::route_layer(...)` only requests that match a route are
//! counted; a request that matches no route is not counted. With a plain
//! `Router::layer(...)`, a request that matches a route gets the route template
//! and a request that reaches the fallback (no matched route) gets the label
//! [`UNMATCHED_OPERATION`]. The latency sum is in seconds, with six decimals.
//!
//! `OperationMetrics` implements [`MetricsProvider`], so a service can append
//! its `render_metrics()` text to its own metrics output.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use metrics_prometheus::{Counter, Label, LabeledSample, Latency, SampleGroup};
use service_observability::MetricsProvider;

/// The operation label of a request that reaches the fallback (no matched route).
pub const UNMATCHED_OPERATION: &str = "unmatched";

/// Series keyed by `(operation, method)`.
type SeriesMap = BTreeMap<(String, String), Arc<OperationSeries>>;

/// Shared per-operation series, keyed by `(operation, method)`.
#[derive(Clone, Default)]
pub struct OperationMetrics {
    series: Arc<Mutex<SeriesMap>>,
}

#[derive(Default)]
struct OperationSeries {
    requests: Counter,
    errors: Counter,
    latency: Latency,
}

/// One row of the snapshot taken by [`OperationMetrics::render_metrics`].
struct Row {
    operation: String,
    method: String,
    requests: u64,
    errors: u64,
    latency_sum_micros: u64,
    latency_count: u64,
}

impl OperationMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, SeriesMap> {
        self.series
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn series(&self, operation: &str, method: &str) -> Arc<OperationSeries> {
        let mut series = self.lock();
        series
            .entry((operation.to_owned(), method.to_owned()))
            .or_default()
            .clone()
    }

    fn snapshot(&self) -> Vec<Row> {
        let series = self.lock();
        series
            .iter()
            .map(|((operation, method), entry)| Row {
                operation: operation.clone(),
                method: method.clone(),
                requests: entry.requests.get(),
                errors: entry.errors.get(),
                latency_sum_micros: entry.latency.sum().get(),
                latency_count: entry.latency.count().get(),
            })
            .collect()
    }
}

impl MetricsProvider for OperationMetrics {
    fn render_metrics(&self) -> String {
        let rows = self.snapshot();
        if rows.is_empty() {
            return String::new();
        }

        let requests: Vec<LabeledSample<'_>> = rows
            .iter()
            .map(|row| {
                LabeledSample::new(
                    vec![
                        Label::new("operation", &row.operation),
                        Label::new("method", &row.method),
                    ],
                    row.requests,
                )
            })
            .collect();
        let errors: Vec<LabeledSample<'_>> = rows
            .iter()
            .map(|row| {
                LabeledSample::new(
                    vec![
                        Label::new("operation", &row.operation),
                        Label::new("method", &row.method),
                    ],
                    row.errors,
                )
            })
            .collect();
        let groups = [
            SampleGroup::new(
                "http_requests_total",
                "counter",
                "Total HTTP requests by operation and method.",
                &requests,
            ),
            SampleGroup::new(
                "http_request_errors_total",
                "counter",
                "HTTP responses with status 500 or higher by operation and method.",
                &errors,
            ),
        ];
        let mut out = metrics_prometheus::render_labeled(&groups);

        out.push_str(
            "# HELP http_request_duration_seconds Request latency in seconds by operation and method.\n",
        );
        out.push_str("# TYPE http_request_duration_seconds summary\n");
        for row in &rows {
            let labels = format!(
                "method=\"{}\",operation=\"{}\"",
                metrics_prometheus::escape_label_value(&row.method),
                metrics_prometheus::escape_label_value(&row.operation),
            );
            out.push_str(&format!(
                "http_request_duration_seconds_sum{{{labels}}} {}\n",
                format_seconds(row.latency_sum_micros)
            ));
            out.push_str(&format!(
                "http_request_duration_seconds_count{{{labels}}} {}\n",
                row.latency_count
            ));
        }
        out
    }
}

/// Count one request under its matched route template and bounded method, and
/// sum its latency. Use it with `route_layer` or `layer`; see the module docs.
pub async fn operation_metrics_middleware(
    State(metrics): State<OperationMetrics>,
    request: Request,
    next: Next,
) -> Response {
    let operation = request
        .extensions()
        .get::<MatchedPath>()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|| UNMATCHED_OPERATION.to_owned());
    let method = bounded_method(request.method());

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed_micros = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);

    let series = metrics.series(&operation, method);
    series.requests.incr();
    if response.status().as_u16() >= 500 {
        series.errors.incr();
    }
    series.latency.observe(elapsed_micros);
    response
}

/// Keeps the method label set bounded: any method outside the standard list is `OTHER`.
fn bounded_method(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::OPTIONS => "OPTIONS",
        Method::TRACE => "TRACE",
        Method::CONNECT => "CONNECT",
        _ => "OTHER",
    }
}

/// Renders microseconds as seconds with exactly six decimals, without floats.
fn format_seconds(micros: u64) -> String {
    format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::format_seconds;

    #[test]
    fn format_seconds_renders_six_decimals() {
        assert_eq!(format_seconds(0), "0.000000");
        assert_eq!(format_seconds(42), "0.000042");
        assert_eq!(format_seconds(1_500_000), "1.500000");
        assert_eq!(format_seconds(12_345_678), "12.345678");
    }
}
