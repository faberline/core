use super::*;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;
use utoipa::OpenApi as _;

#[derive(utoipa::OpenApi)]
#[openapi(info(title = "test", description = "probe-route test doc"))]
struct TestDoc;

fn test_openapi() -> utoipa::openapi::OpenApi {
    TestDoc::openapi()
}

struct Draining(bool);
impl ReadinessHook for Draining {
    fn is_draining(&self) -> bool {
        self.0
    }
}

struct StaticMetrics(&'static str);
impl MetricsProvider for StaticMetrics {
    fn render_metrics(&self) -> String {
        self.0.to_string()
    }
}

async fn get(router: Router, path: &str) -> (StatusCode, String) {
    let resp = router
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn healthz_is_ok() {
    let router = standard_probe_routes(Arc::new(Draining(false)), None, test_openapi);
    let (status, body) = get(router, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn readyz_200_when_not_draining() {
    let router = standard_probe_routes(Arc::new(Draining(false)), None, test_openapi);
    let (status, body) = get(router, "/readyz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn readyz_503_when_draining() {
    let router = standard_probe_routes(Arc::new(Draining(true)), None, test_openapi);
    let (status, body) = get(router, "/readyz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body, "draining");
}

#[tokio::test]
async fn metrics_renders_provider_text() {
    let metrics: Arc<dyn MetricsProvider> = Arc::new(StaticMetrics("svc_up 1\n"));
    let router = standard_probe_routes(Arc::new(Draining(false)), Some(metrics), test_openapi);
    let (status, body) = get(router, "/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "svc_up 1\n");
}

#[tokio::test]
async fn metrics_empty_when_no_provider() {
    let router = standard_probe_routes(Arc::new(Draining(false)), None, test_openapi);
    let (status, body) = get(router, "/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "");
}

#[tokio::test]
async fn openapi_json_parses() {
    let router = standard_probe_routes(Arc::new(Draining(false)), None, test_openapi);
    let (status, body) = get(router, "/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["info"]["title"], "test");
}

#[tokio::test]
async fn canonical_openapi_keeps_producer_bytes() {
    fn canonical() -> String {
        "{\n  \"openapi\": \"3.2.0\"\n}".into()
    }
    let router = standard_probe_routes_canonical_json(Arc::new(Draining(false)), None, canonical);
    let (status, body) = get(router, "/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, canonical());
}

#[tokio::test]
async fn docs_serves_swagger_page() {
    let router = standard_probe_routes(Arc::new(Draining(false)), None, test_openapi);
    let (status, body) = get(router, "/docs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("swagger-ui"));
    assert!(body.contains("/openapi.json"));
}
