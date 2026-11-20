// HANDWRITE-BEGIN gap="missing-generator:unit-test:7c2e5b10" tracker="#560" reason="Live-router coverage for the opt-in request id middleware: echo, generation, replacement, extension and JSON log field."
use std::io;
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::Body;
use axum::extract::Extension;
use axum::http::{HeaderValue, Request};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use service_http::{request_id_middleware, RequestId, REQUEST_ID_HEADER};
use service_observability::{ServiceIdentity, ServiceJsonFormatter, ServiceLogEventV1};
use tower::ServiceExt as _;
use tracing_subscriber::fmt::format::JsonFields;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;

async fn echo_handler(Extension(id): Extension<RequestId>) -> String {
    id.as_str().to_string()
}

async fn logging_handler() -> &'static str {
    tracing::info!(event = "handled", "handled");
    "ok"
}

fn app() -> Router {
    Router::new()
        .route("/", get(echo_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
}

async fn send(app: Router, value: Option<HeaderValue>) -> Response {
    let mut builder = Request::builder().uri("/");
    if let Some(value) = value {
        builder = builder.header(REQUEST_ID_HEADER, value);
    }
    app.oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn header_of(response: &Response) -> String {
    response
        .headers()
        .get(REQUEST_ID_HEADER)
        .expect("x-request-id response header")
        .to_str()
        .unwrap()
        .to_string()
}

fn is_hex32(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

async fn body_of(response: Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn valid_header_is_echoed() {
    let response = send(app(), Some(HeaderValue::from_static("abc"))).await;
    assert_eq!(header_of(&response), "abc");
    assert_eq!(body_of(response).await, "abc");
}

#[tokio::test(flavor = "current_thread")]
async fn missing_header_gets_a_new_id() {
    let first = header_of(&send(app(), None).await);
    let second = header_of(&send(app(), None).await);
    assert!(is_hex32(&first), "not 32 hex characters: {first:?}");
    assert!(is_hex32(&second), "not 32 hex characters: {second:?}");
    assert_ne!(first, second);
}

#[tokio::test(flavor = "current_thread")]
async fn too_long_header_is_replaced() {
    let long = "a".repeat(129);
    let response = send(app(), Some(HeaderValue::from_str(&long).unwrap())).await;
    let id = header_of(&response);
    assert_ne!(id, long);
    assert!(is_hex32(&id), "not 32 hex characters: {id:?}");

    let exact = "a".repeat(128);
    let kept = send(app(), Some(HeaderValue::from_str(&exact).unwrap())).await;
    assert_eq!(header_of(&kept), exact);
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_header_is_replaced() {
    let spaced = send(app(), Some(HeaderValue::from_static("bad id"))).await;
    let id = header_of(&spaced);
    assert_ne!(id, "bad id");
    assert!(is_hex32(&id), "not 32 hex characters: {id:?}");

    let non_utf8 = HeaderValue::from_bytes(&[0xff, b'a']).unwrap();
    let id = header_of(&send(app(), Some(non_utf8)).await);
    assert!(is_hex32(&id), "not 32 hex characters: {id:?}");

    let empty = header_of(&send(app(), Some(HeaderValue::from_static(""))).await);
    assert!(is_hex32(&empty), "not 32 hex characters: {empty:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn extension_matches_response_header() {
    let response = send(app(), None).await;
    let id = header_of(&response);
    assert_eq!(body_of(response).await, id);
}

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

struct SharedWriterGuard(Arc<Mutex<Vec<u8>>>);

impl io::Write for SharedWriterGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for SharedWriter {
    type Writer = SharedWriterGuard;

    fn make_writer(&'writer self) -> Self::Writer {
        SharedWriterGuard(self.0.clone())
    }
}

impl SharedWriter {
    fn output(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

/// The JSON log capture of this test binary. It is the global default, set
/// once: a thread-local subscriber races with other tests, which can cache a
/// "never" interest for the span and event callsites while no subscriber is
/// set, and then the span is dropped from the log line.
fn log_writer() -> SharedWriter {
    static LOG_WRITER: OnceLock<SharedWriter> = OnceLock::new();
    LOG_WRITER
        .get_or_init(|| {
            let writer = SharedWriter::default();
            let identity = ServiceIdentity::new("fixture-service", "1.0.0").unwrap();
            let layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .fmt_fields(JsonFields::new())
                .event_format(ServiceJsonFormatter::new(identity))
                .with_writer(writer.clone());
            let subscriber = tracing_subscriber::registry().with(layer);
            tracing::subscriber::set_global_default(subscriber)
                .expect("no other global subscriber in this test binary");
            writer
        })
        .clone()
}

#[tokio::test(flavor = "current_thread")]
async fn id_is_in_the_json_log_field() {
    let writer = log_writer();

    let logging_app = Router::new()
        .route("/", get(logging_handler))
        .layer(axum::middleware::from_fn(request_id_middleware));

    let first = send(logging_app.clone(), Some(HeaderValue::from_static("abc"))).await;
    assert_eq!(header_of(&first), "abc");
    let second = send(logging_app, None).await;
    let generated = header_of(&second);
    assert!(is_hex32(&generated), "not 32 hex characters: {generated:?}");

    let output = writer.output();
    let records: Vec<ServiceLogEventV1> = output
        .lines()
        .filter_map(|line| serde_json::from_str::<ServiceLogEventV1>(line).ok())
        .collect();
    let handled: Vec<&ServiceLogEventV1> = records
        .iter()
        .filter(|record| record.event == "handled")
        .collect();
    assert_eq!(handled.len(), 2);
    assert_eq!(handled[0].request_id.as_deref(), Some("abc"));
    assert_eq!(handled[1].request_id.as_deref(), Some(generated.as_str()));
}
// HANDWRITE-END
