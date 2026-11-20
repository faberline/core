// HANDWRITE-BEGIN gap="missing-generator:unit-test:5d2f7c13" tracker="#562" reason="Live coverage for catalog error codes, the details array wire form and the request timeout middleware."
use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use http_body_util::BodyExt;
use service_http::{error_code, request_timeout_middleware, RequestTimeout};
use tower::ServiceExt as _;

async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_secs(5)).await;
    "late"
}

fn app(limit: Duration) -> Router {
    Router::new()
        .route("/slow", get(slow))
        .route("/fast", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn_with_state(
            RequestTimeout::new(limit),
            request_timeout_middleware,
        ))
}

async fn get_path(app: Router, path: &str) -> axum::response::Response {
    tokio::time::timeout(
        Duration::from_secs(2),
        app.oneshot(Request::get(path).body(Body::empty()).unwrap()),
    )
    .await
    .expect("the request finished in time")
    .unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn slow_handler_gets_503_unavailable() {
    let response = get_path(app(Duration::from_millis(50)), "/slow").await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "error": "unavailable",
            "message": "request timed out after 50 ms",
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn fast_handler_is_not_changed() {
    let response = get_path(app(Duration::from_secs(5)), "/fast").await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"ok");
}

#[tokio::test(flavor = "current_thread")]
async fn unavailable_code_is_the_catalog_constant() {
    let response = get_path(app(Duration::from_millis(50)), "/slow").await;
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"], error_code::UNAVAILABLE);
}
// HANDWRITE-END
