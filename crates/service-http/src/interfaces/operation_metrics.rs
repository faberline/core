//! Opt-in per-operation request metrics.
//!
//! [`operation_metrics_middleware`] counts requests and 5xx errors and sums
//! the latency of each request, per matched route template and method. The
//! operation is the axum route template (`MatchedPath`), so the label set stays
//! small. Add it with `route_layer` to count only requests that match a route.
//! With a plain `Router::layer`, a request that reaches the fallback (no
//! matched route) gets the label [`UNMATCHED_OPERATION`]. An
//! [`OperationMetrics`] implements `MetricsProvider`, so a service can pass it
//! to its probe routes or append its `render_metrics()` text to its own.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use metrics_prometheus::{
    escape_label_value, render_labeled, Counter, Label, LabeledSample, Latency, SampleGroup,
};
use service_observability::MetricsProvider;

/// The operation label of a request that matches no route.
pub const UNMATCHED_OPERATION: &str = "unmatched";

#[derive(Default)]
struct OperationSeries {
    requests: Counter,
    errors: Counter,
    latency: Latency,
}

/// Per-operation request counters and latency sums, rendered as Prometheus
/// text. Build one, pass a clone to [`operation_metrics_middleware`], and
/// render it through `MetricsProvider`.
/// Series by `(operation, method)`, sorted so the rendered text is stable.
type SeriesMap = BTreeMap<(String, String), Arc<OperationSeries>>;

#[derive(Clone, Default)]
pub struct OperationMetrics {
    series: Arc<Mutex<SeriesMap>>,
}

impl OperationMetrics {
    /// An empty set of metrics. It renders nothing until a request is counted.
    pub fn new() -> Self {
        Self::default()
    }

    fn series(&self, operation: &str, method: &str) -> Arc<OperationSeries> {
        let mut map = self
            .series
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.entry((operation.to_owned(), method.to_owned()))
            .or_default()
            .clone()
    }
}

/// Counts one request in `metrics`: its requests, its errors (status 500 or
/// higher) and its latency in microseconds. The operation is the matched route
/// template, or [`UNMATCHED_OPERATION`]. Use `route_layer` to count only
/// matched requests. Use it through `axum::middleware::from_fn_with_state`.
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
    let method = known_method(request.method().as_str());
    let start = Instant::now();
    let response = next.run(request).await;
    let series = metrics.series(&operation, method);
    series.requests.incr();
    if response.status().as_u16() >= 500 {
        series.errors.incr();
    }
    series
        .latency
        .observe(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX));
    response
}

/// The standard methods that have their own label.
const KNOWN_METHODS: [&str; 9] = [
    "GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "TRACE", "CONNECT",
];

/// Keeps the method label set small: any method that is not standard is `OTHER`.
fn known_method(method: &str) -> &'static str {
    KNOWN_METHODS
        .iter()
        .copied()
        .find(|known| *known == method)
        .unwrap_or("OTHER")
}

impl MetricsProvider for OperationMetrics {
    fn render_metrics(&self) -> String {
        let snapshot: Vec<(String, String, u64, u64, u64, u64)> = {
            let map = self
                .series
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            map.iter()
                .map(|((operation, method), series)| {
                    (
                        operation.clone(),
                        method.clone(),
                        series.requests.get(),
                        series.errors.get(),
                        series.latency.sum().get(),
                        series.latency.count().get(),
                    )
                })
                .collect()
        };
        if snapshot.is_empty() {
            return String::new();
        }

        let requests: Vec<LabeledSample<'_>> = snapshot
            .iter()
            .map(|(operation, method, requests, _, _, _)| {
                LabeledSample::new(
                    vec![
                        Label::new("operation", operation),
                        Label::new("method", method),
                    ],
                    *requests,
                )
            })
            .collect();
        let errors: Vec<LabeledSample<'_>> = snapshot
            .iter()
            .map(|(operation, method, _, errors, _, _)| {
                LabeledSample::new(
                    vec![
                        Label::new("operation", operation),
                        Label::new("method", method),
                    ],
                    *errors,
                )
            })
            .collect();

        let mut out = render_labeled(&[
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
        ]);

        out.push_str(
            "# HELP http_request_duration_seconds Request latency in seconds by operation and method.\n",
        );
        out.push_str("# TYPE http_request_duration_seconds summary\n");
        for (operation, method, _, _, sum_micros, count) in &snapshot {
            let labels = format!(
                "method=\"{}\",operation=\"{}\"",
                escape_label_value(method),
                escape_label_value(operation)
            );
            out.push_str(&format!(
                "http_request_duration_seconds_sum{{{labels}}} {}\n",
                format_seconds(*sum_micros)
            ));
            out.push_str(&format!(
                "http_request_duration_seconds_count{{{labels}}} {count}\n"
            ));
        }
        out
    }
}

/// Formats microseconds as seconds with six decimals, without floating point.
fn format_seconds(micros: u64) -> String {
    format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::format_seconds;

    #[test]
    fn format_seconds_is_exact() {
        assert_eq!(format_seconds(0), "0.000000");
        assert_eq!(format_seconds(42), "0.000042");
        assert_eq!(format_seconds(1_500_000), "1.500000");
        assert_eq!(format_seconds(12_345_678), "12.345678");
    }
}
