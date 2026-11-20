// HANDWRITE-BEGIN gap="missing-generator:unit-test:3b9d41e8" tracker="#561" reason="Live-router coverage for the opt-in per-operation metrics middleware: route template label, 5xx errors, cardinality, unmatched, method bound and exact rendering."
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Method, Request, StatusCode};
use axum::routing::{any, get};
use axum::Router;
use service_http::{operation_metrics_middleware, OperationMetrics};
use service_observability::MetricsProvider as _;
use tower::ServiceExt as _;

async fn item(Path(id): Path<String>) -> StatusCode {
    if id == "bad" {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    }
}

fn app(metrics: &OperationMetrics) -> Router {
    Router::new()
        .route("/items/{id}", get(item))
        .route("/any", any(|| async { "ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            metrics.clone(),
            operation_metrics_middleware,
        ))
}

async fn get_path(app: &Router, path: &str) {
    send(app, Method::GET, path).await;
}

async fn send(app: &Router, method: Method, path: &str) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(request).await.unwrap();
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|l| l == line)
}

fn lines_starting<'a>(text: &'a str, prefix: &str) -> Vec<&'a str> {
    text.lines().filter(|l| l.starts_with(prefix)).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn counts_requests_and_errors_by_route_template() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);
    get_path(&app, "/items/1").await;
    get_path(&app, "/items/bad").await;

    let text = metrics.render_metrics();
    assert!(has_line(
        &text,
        r#"http_requests_total{method="GET",operation="/items/{id}"} 2"#
    ));
    assert!(has_line(
        &text,
        r#"http_request_errors_total{method="GET",operation="/items/{id}"} 1"#
    ));
    assert!(has_line(
        &text,
        r#"http_request_duration_seconds_count{method="GET",operation="/items/{id}"} 2"#
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn different_ids_share_one_label_set() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);
    get_path(&app, "/items/1").await;
    get_path(&app, "/items/2").await;
    get_path(&app, "/items/3").await;

    let text = metrics.render_metrics();
    let requests = lines_starting(&text, "http_requests_total{");
    assert_eq!(requests.len(), 1, "one label set expected: {requests:?}");
    assert!(requests[0].ends_with(" 3"), "{}", requests[0]);

    let errors = lines_starting(&text, "http_request_errors_total{");
    assert_eq!(errors.len(), 1, "one error label set expected: {errors:?}");
    assert!(errors[0].ends_with(" 0"), "{}", errors[0]);
}

#[tokio::test(flavor = "current_thread")]
async fn unmatched_label_is_the_fallback_request() {
    // Plain `Router::layer` (not `route_layer`): a matched route keeps its
    // template label, and only the request that reaches the fallback is
    // `unmatched`.
    let metrics = OperationMetrics::new();
    let plain =
        Router::new()
            .route("/items/{id}", get(item))
            .layer(axum::middleware::from_fn_with_state(
                metrics.clone(),
                operation_metrics_middleware,
            ));
    get_path(&plain, "/items/1").await;
    get_path(&plain, "/nope").await;

    let text = metrics.render_metrics();
    assert!(has_line(
        &text,
        r#"http_requests_total{method="GET",operation="/items/{id}"} 1"#
    ));
    assert!(has_line(
        &text,
        r#"http_requests_total{method="GET",operation="unmatched"} 1"#
    ));
    assert!(has_line(
        &text,
        r#"http_request_errors_total{method="GET",operation="unmatched"} 0"#
    ));

    // With `route_layer`, a request that matches no route is not counted.
    let route_metrics = OperationMetrics::new();
    let routed = app(&route_metrics);
    get_path(&routed, "/nope").await;
    assert_eq!(route_metrics.render_metrics(), "");
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_method_is_bounded() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);
    send(&app, Method::POST, "/any").await;
    send(&app, Method::from_bytes(b"BREW").unwrap(), "/any").await;

    let text = metrics.render_metrics();
    assert!(has_line(
        &text,
        r#"http_requests_total{method="POST",operation="/any"} 1"#
    ));
    assert!(has_line(
        &text,
        r#"http_requests_total{method="OTHER",operation="/any"} 1"#
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn duration_sum_is_seconds_with_six_decimals() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);
    get_path(&app, "/items/1").await;

    let text = metrics.render_metrics();
    let prefix = r#"http_request_duration_seconds_sum{method="GET",operation="/items/{id}"} "#;
    let sums = lines_starting(&text, prefix);
    assert_eq!(sums.len(), 1, "one sum line expected: {sums:?}");
    let value = &sums[0][prefix.len()..];
    let (whole, fraction) = value
        .split_once('.')
        .unwrap_or_else(|| panic!("sum has no decimal point: {value}"));
    whole
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("integer part is not a number: {value}"));
    assert_eq!(fraction.len(), 6, "six decimals expected: {value}");
    assert!(
        fraction.bytes().all(|b| b.is_ascii_digit()),
        "fraction must be digits: {value}"
    );
    let seconds: f64 = value.parse().unwrap();
    assert!(seconds < 5.0, "one local request took {seconds} s");
}

#[tokio::test(flavor = "current_thread")]
async fn empty_metrics_render_nothing_and_do_not_change_other_providers() {
    assert_eq!(OperationMetrics::new().render_metrics(), "");

    // An app without the middleware must not touch a separate instance.
    let unused = OperationMetrics::new();
    let plain = Router::new().route("/items/{id}", get(item));
    get_path(&plain, "/items/1").await;
    assert_eq!(unused.render_metrics(), "");
}
// HANDWRITE-END
