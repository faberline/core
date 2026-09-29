use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::application::{AdmissionController, AdmissionInput};
use crate::ApiErr;

type Classifier = dyn Fn(&Request) -> Option<AdmissionInput> + Send + Sync;

/// State consumed by [`admission_middleware`]. Apps own the classifier;
/// enforcement remains shared.
#[derive(Clone)]
pub struct AdmissionMiddleware {
    controller: AdmissionController,
    classifier: Arc<Classifier>,
}

impl AdmissionMiddleware {
    pub fn new<F>(controller: AdmissionController, classifier: F) -> Self
    where
        F: Fn(&Request) -> Option<AdmissionInput> + Send + Sync + 'static,
    {
        Self {
            controller,
            classifier: Arc::new(classifier),
        }
    }
}

pub async fn admission_middleware(
    State(state): State<AdmissionMiddleware>,
    request: Request,
    next: Next,
) -> Response {
    let Some(input) = (state.classifier)(&request) else {
        return next.run(request).await;
    };
    let decision = state.controller.admit(&input);
    if decision.is_allowed() {
        return next.run(request).await;
    }

    let retry_after = decision.retry_after.unwrap_or(Duration::from_secs(1));
    let retry_seconds = retry_after
        .as_secs()
        .saturating_add(u64::from(retry_after.subsec_nanos() > 0));
    let mut response = ApiErr::new(
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "request admission limit exceeded",
    )
    .into_response();
    response.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_str(&retry_seconds.max(1).to_string())
            .expect("retry-after seconds are valid header text"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        AdmissionEvent, AdmissionObserver, AdmissionOutcome, AdmissionPolicy,
    };
    use std::sync::Mutex;

    use axum::body::Body;
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[derive(Default)]
    struct RecordingObserver(Mutex<Vec<AdmissionEvent>>);

    impl AdmissionObserver for RecordingObserver {
        fn record(&self, event: &AdmissionEvent) {
            self.0.lock().unwrap().push(event.clone());
        }
    }

    fn policy(capacity: u32, max_keys: usize) -> AdmissionPolicy {
        AdmissionPolicy::new(capacity, Duration::from_secs(10), max_keys).unwrap()
    }

    #[test]
    fn allow_deny_and_refill_are_deterministic() {
        let controller = AdmissionController::new([("read", policy(2, 8))]);
        let input = AdmissionInput::new("read", b"opaque-secret");
        assert_eq!(
            controller.admit_at(&input, Duration::ZERO).outcome,
            AdmissionOutcome::Allow
        );
        assert_eq!(
            controller.admit_at(&input, Duration::ZERO).outcome,
            AdmissionOutcome::Allow
        );
        let denied = controller.admit_at(&input, Duration::ZERO);
        assert_eq!(denied.outcome, AdmissionOutcome::Deny);
        assert_eq!(denied.retry_after, Some(Duration::from_secs(5)));
        assert_eq!(
            controller.admit_at(&input, Duration::from_secs(5)).outcome,
            AdmissionOutcome::Allow
        );
    }

    #[test]
    fn state_is_bounded_and_observer_schema_is_key_free() {
        let observer = Arc::new(RecordingObserver::default());
        let controller =
            AdmissionController::with_observer([("write", policy(1, 2))], observer.clone());
        for key in ["secret-a", "secret-b", "secret-c"] {
            controller.admit_at(
                &AdmissionInput::new("write", key.as_bytes()),
                Duration::ZERO,
            );
        }
        assert_eq!(controller.tracked_keys("write"), 2);
        let json = serde_json::to_string(&*observer.0.lock().unwrap()).unwrap();
        assert!(!json.contains("secret-a"));
        assert!(!json.contains("secret-b"));
        assert!(!json.contains("secret-c"));
        assert!(!json.contains("fingerprint"));
        assert!(!json.contains("credential"));
    }

    #[test]
    fn unconfigured_class_bypasses_without_allocating_state() {
        let controller = AdmissionController::new([("read", policy(1, 1))]);
        let decision = controller.admit_at(
            &AdmissionInput::new("unconfigured", b"anything"),
            Duration::ZERO,
        );
        assert_eq!(decision.outcome, AdmissionOutcome::Bypass);
        assert_eq!(controller.tracked_keys("unconfigured"), 0);
    }

    #[tokio::test]
    async fn middleware_returns_standard_429_and_retry_after() {
        let controller = AdmissionController::new([("read", policy(1, 1))]);
        let middleware = AdmissionMiddleware::new(controller, |_| {
            Some(AdmissionInput::new("read", b"anonymous"))
        });
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .route_layer(axum::middleware::from_fn_with_state(
                middleware,
                admission_middleware,
            ));
        let request = || Request::builder().uri("/").body(Body::empty()).unwrap();
        assert_eq!(
            app.clone().oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );
        let denied = app.oneshot(request()).await.unwrap();
        assert_eq!(denied.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(denied.headers()[header::RETRY_AFTER], "10");
        let body = denied.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "rate_limited");
    }
}
