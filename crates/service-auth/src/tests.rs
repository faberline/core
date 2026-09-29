use std::sync::Arc;

use axum::{
    body::Body,
    extract::Extension,
    http::{header, HeaderMap, Request, StatusCode},
    middleware::from_fn_with_state,
    routing::get,
    Router,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::*;

// ---- A tiny test verifier -------------------------------------------

/// The service's concrete principal. With a real service this would carry
/// the subject/roles/scope; the unit variant keeps the test minimal.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TestPrincipal {
    Authed(String),
    Open,
}

/// Fixed-token verifier: only `Bearer good` authenticates. `required`
/// controls open-mode; when `false`, a request without a token resolves to
/// `TestPrincipal::Open`.
struct FixedToken {
    required: bool,
}

impl Verifier for FixedToken {
    type Principal = TestPrincipal;

    fn authenticate(&self, headers: &HeaderMap) -> Result<TestPrincipal, AuthError> {
        match bearer_token(headers) {
            Some("good") => Ok(TestPrincipal::Authed("subject".into())),
            Some(_) => Err(AuthError::Unauthenticated),
            None if !self.required => Ok(TestPrincipal::Open),
            None => Err(AuthError::Unauthenticated),
        }
    }

    fn required(&self) -> bool {
        self.required
    }
}

/// A verifier that always 403s — exercises the Forbidden arm.
struct AlwaysForbidden;

impl Verifier for AlwaysForbidden {
    type Principal = TestPrincipal;

    fn authenticate(&self, _headers: &HeaderMap) -> Result<TestPrincipal, AuthError> {
        Err(AuthError::Forbidden("nope".into()))
    }
}

/// Probe handler: reflects the injected principal so tests can assert it
/// reached the handler concretely (no downcast).
async fn probe(Extension(p): Extension<TestPrincipal>) -> String {
    match p {
        TestPrincipal::Authed(s) => format!("authed:{s}"),
        TestPrincipal::Open => "open".into(),
    }
}

fn app<V: Verifier<Principal = TestPrincipal>>(verifier: V) -> Router {
    Router::new()
        .route("/", get(probe))
        .layer(from_fn_with_state(Arc::new(verifier), auth_middleware::<V>))
}

async fn call(app: Router, auth: Option<&str>) -> (StatusCode, String) {
    let mut builder = Request::builder().uri("/");
    if let Some(a) = auth {
        builder = builder.header(header::AUTHORIZATION, a);
    }
    let resp = app
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

// ---- bearer_token unit test -----------------------------------------

#[test]
fn bearer_token_extracts_and_rejects() {
    let mut h = HeaderMap::new();
    h.insert(header::AUTHORIZATION, "Bearer abc123".parse().unwrap());
    assert_eq!(bearer_token(&h), Some("abc123"));

    // Wrong scheme / no scheme / missing header all yield None.
    let mut basic = HeaderMap::new();
    basic.insert(header::AUTHORIZATION, "Basic abc123".parse().unwrap());
    assert_eq!(bearer_token(&basic), None);

    let mut raw = HeaderMap::new();
    raw.insert(header::AUTHORIZATION, "abc123".parse().unwrap());
    assert_eq!(bearer_token(&raw), None);

    assert_eq!(bearer_token(&HeaderMap::new()), None);
}

// ---- middleware integration tests (oneshot) -------------------------

#[tokio::test]
async fn valid_bearer_injects_principal_and_runs_handler() {
    let (status, body) = call(app(FixedToken { required: true }), Some("Bearer good")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "authed:subject");
}

#[tokio::test]
async fn missing_token_when_required_is_401() {
    let (status, _body) = call(app(FixedToken { required: true }), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_token_is_401() {
    let (status, _body) = call(app(FixedToken { required: true }), Some("Bearer bad")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn open_mode_without_token_injects_open_principal() {
    let (status, body) = call(app(FixedToken { required: false }), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "open");
}

#[tokio::test]
async fn forbidden_is_403() {
    let (status, body) = call(app(AlwaysForbidden), Some("Bearer good")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Error body uses the shared {"error","message"} shape.
    assert!(body.contains("\"error\":\"forbidden\""));
    assert!(body.contains("nope"));
}
