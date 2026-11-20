// HANDWRITE-BEGIN gap="missing-generator:unit-test:3b9d41e8" tracker="#561" reason="Live-router coverage for the opt-in per-operation metrics middleware: route template label, 5xx errors, cardinality, unmatched, method bound and exact rendering."
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Method, Request, StatusCode};
use axum::routing::{any, get};
use axum::Router;
use service_http::{operation_metrics_middleware, OperationMetrics};
use service_observability::MetricsProvider;
use tower::ServiceExt;

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

async fn send(app: &Router, method: Method, path: &str) -> StatusCode {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .expect("request");
    app.clone()
        .oneshot(request)
        .await
        .expect("response")
        .status()
}

async fn get_path(app: &Router, path: &str) -> StatusCode {
    send(app, Method::GET, path).await
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|candidate| candidate == line)
}

#[tokio::test(flavor = "current_thread")]
async fn counts_requests_and_errors_by_route_template() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);

    assert_eq!(get_path(&app, "/items/1").await, StatusCode::OK);
    assert_eq!(
        get_path(&app, "/items/bad").await,
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let text = metrics.render_metrics();
    assert!(
        has_line(
            &text,
            r#"http_requests_total{method="GET",operation="/items/{id}"} 2"#
        ),
        "{text}"
    );
    assert!(
        has_line(
            &text,
            r#"http_request_errors_total{method="GET",operation="/items/{id}"} 1"#
        ),
        "{text}"
    );
    assert!(
        has_line(
            &text,
            r#"http_request_duration_seconds_count{method="GET",operation="/items/{id}"} 2"#
        ),
        "{text}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn different_ids_share_one_label_set() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);

    for path in ["/items/1", "/items/2", "/items/3"] {
        assert_eq!(get_path(&app, path).await, StatusCode::OK);
    }

    let text = metrics.render_metrics();
    let request_lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("http_requests_total{"))
        .collect();
    assert_eq!(request_lines.len(), 1, "{text}");
    assert!(request_lines[0].ends_with(" 3"), "{text}");

    let error_line = text
        .lines()
        .find(|line| line.starts_with("http_request_errors_total{"))
        .expect("error line");
    assert!(error_line.ends_with(" 0"), "{text}");
}

#[tokio::test(flavor = "current_thread")]
async fn unmatched_label_is_the_fallback_request() {
    // Plain `layer` wraps every route and the fallback: a matched request keeps
    // its route template, and a request that reaches the fallback is `unmatched`.
    let metrics = OperationMetrics::new();
    let layered =
        Router::new()
            .route("/items/{id}", get(item))
            .layer(axum::middleware::from_fn_with_state(
                metrics.clone(),
                operation_metrics_middleware,
            ));

    assert_eq!(get_path(&layered, "/items/1").await, StatusCode::OK);
    assert_eq!(get_path(&layered, "/nope").await, StatusCode::NOT_FOUND);

    let text = metrics.render_metrics();
    assert!(
        has_line(
            &text,
            r#"http_requests_total{method="GET",operation="/items/{id}"} 1"#
        ),
        "{text}"
    );
    assert!(
        has_line(
            &text,
            r#"http_requests_total{method="GET",operation="unmatched"} 1"#
        ),
        "{text}"
    );
    assert!(
        has_line(
            &text,
            r#"http_request_errors_total{method="GET",operation="unmatched"} 0"#
        ),
        "{text}"
    );

    // With `route_layer`, a request that matches no route is not counted at all.
    let route_metrics = OperationMetrics::new();
    let route_app = app(&route_metrics);
    assert_eq!(get_path(&route_app, "/nope").await, StatusCode::NOT_FOUND);
    assert_eq!(route_metrics.render_metrics(), "");
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_method_is_bounded() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);

    assert_eq!(send(&app, Method::POST, "/any").await, StatusCode::OK);
    let brew = Method::from_bytes(b"BREW").expect("extension method");
    send(&app, brew, "/any").await;

    let text = metrics.render_metrics();
    assert!(
        has_line(
            &text,
            r#"http_requests_total{method="POST",operation="/any"} 1"#
        ),
        "{text}"
    );
    assert!(
        has_line(
            &text,
            r#"http_requests_total{method="OTHER",operation="/any"} 1"#
        ),
        "{text}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn duration_sum_is_seconds_with_six_decimals() {
    let metrics = OperationMetrics::new();
    let app = app(&metrics);
    assert_eq!(get_path(&app, "/items/1").await, StatusCode::OK);

    let text = metrics.render_metrics();
    let line = text
        .lines()
        .find(|line| {
            line.starts_with(
                r#"http_request_duration_seconds_sum{method="GET",operation="/items/{id}"} "#,
            )
        })
        .expect("duration sum line");
    let value = line.rsplit(' ').next().expect("value");

    let (integer, fraction) = value.split_once('.').expect("decimal point");
    integer.parse::<u64>().expect("integer seconds");
    assert_eq!(fraction.len(), 6, "{value}");
    assert!(
        fraction.bytes().all(|byte| byte.is_ascii_digit()),
        "{value}"
    );
    let seconds: f64 = value.parse().expect("float");
    assert!(seconds < 5.0, "{value}");
}

#[tokio::test(flavor = "current_thread")]
async fn empty_metrics_render_nothing_and_do_not_change_other_providers() {
    assert_eq!(OperationMetrics::new().render_metrics(), "");

    // An app built without the middleware never touches this collector.
    let untouched = OperationMetrics::new();
    let plain = Router::new().route("/items/{id}", get(item));
    assert_eq!(get_path(&plain, "/items/1").await, StatusCode::OK);
    assert_eq!(untouched.render_metrics(), "");
}
// HANDWRITE-END
