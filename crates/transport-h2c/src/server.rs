//! Server-side h2c transport (behind the `server` feature): serve one accepted
//! stream as **HTTP/1.1 or HTTP/2 cleartext (h2c, prior-knowledge)** via
//! hyper-util's auto builder.
//!
//! `axum::serve` speaks HTTP/1 only; this is the ecosystem's drop-in replacement
//! so a service actually accepts h2c (the in-cluster default) alongside HTTP/1.1
//! on a single port. The client side of the same transport lives in this crate's
//! `h2c_client` / `H2cPool` / `H2cManager`.

mod accounting;

use accounting::{observe_drain, Accounting};
use http::{Request, Response, StatusCode, Version};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use server_lifecycle::{LifecycleSubscription, ShutdownDeadline};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tower::ServiceExt;

/// Per-connection h2c tuning owned by the transport layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionOptions {
    max_concurrent_streams: u32,
}

impl ConnectionOptions {
    /// Options that cap each connection at `max_concurrent_streams` HTTP/2
    /// streams.
    pub const fn new(max_concurrent_streams: u32) -> Self {
        Self {
            max_concurrent_streams,
        }
    }

    /// The HTTP/2 stream cap per connection.
    pub const fn max_concurrent_streams(&self) -> u32 {
        self.max_concurrent_streams
    }
}

pub type ConnectionError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionProtocol {
    Undetermined,
    Http1,
    Http2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionTerminal {
    PeerClosed,
    Drained,
    DeadlineExceeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionReport {
    pub protocol: ConnectionProtocol,
    pub admitted: usize,
    pub active_at_drain: usize,
    pub completed: usize,
    pub refused: usize,
    pub timed_out: usize,
    pub ambiguous: usize,
    pub terminal: ConnectionTerminal,
    pub error: Option<String>,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 4096,
        }
    }
}

/// Serve one accepted stream as HTTP/1.1 or h2c and dispatch every request
/// through the axum `app`. Listener admission, shutdown, and task supervision
/// belong to `server-http`/`server-tcp`.
pub async fn serve_connection(stream: TcpStream, app: axum::Router) -> Result<(), ConnectionError> {
    serve_io(stream, app).await
}

/// Like [`serve_connection`], with a tunable HTTP/2 stream limit.
pub async fn serve_connection_with_options(
    stream: TcpStream,
    app: axum::Router,
    options: ConnectionOptions,
) -> Result<(), ConnectionError> {
    serve_io_with_options(stream, app, options).await
}

/// Serve one arbitrary Tokio byte stream as HTTP/1.1 or HTTP/2. This is the
/// transport seam used by authenticated peer ports after rustls completes its
/// handshake; cleartext callers continue to use [`serve_connection`].
pub async fn serve_io<I>(stream: I, app: axum::Router) -> Result<(), ConnectionError>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    serve_io_with_options(stream, app, ConnectionOptions::default()).await
}

pub async fn serve_io_with_options<I>(
    stream: I,
    app: axum::Router,
    options: ConnectionOptions,
) -> Result<(), ConnectionError>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut builder = auto::Builder::new(TokioExecutor::new());
    // Lift the per-connection concurrent-stream ceiling: clients open
    // ~ln(concurrency) connections and multiplex many streams over each (see
    // this crate's connection-count heuristic). The hyper default (~200) caused
    // stream starvation / hangs at few-connections + high-concurrency. Flow-
    // control windows stay at hyper defaults — on a low-RTT link the workload is
    // CPU-bound (framing + JSON), not window-bound, so enlarging them is a
    // WAN-only tuning with no local benefit.
    builder
        .http2()
        .max_concurrent_streams(options.max_concurrent_streams());

    let io = TokioIo::new(stream);
    // axum's Router is Service<Request<Incoming>>; oneshot drives one request.
    let svc = service_fn(move |req| app.clone().oneshot(req));
    builder.serve_connection_with_upgrades(io, svc).await
}

pub async fn serve_connection_with_drain(
    stream: TcpStream,
    app: axum::Router,
    options: ConnectionOptions,
    lifecycle: LifecycleSubscription,
    deadline: ShutdownDeadline,
) -> ConnectionReport {
    serve_io_with_drain(stream, app, options, lifecycle, deadline).await
}

/// Serve immediately while the lifecycle is Serving, then drain against the
/// absolute deadline published by that same lifecycle subscription.
pub async fn serve_connection_with_lifecycle(
    stream: TcpStream,
    app: axum::Router,
    options: ConnectionOptions,
    lifecycle: LifecycleSubscription,
) -> ConnectionReport {
    serve_io_with_lifecycle(stream, app, options, lifecycle).await
}

pub async fn serve_io_with_lifecycle<I>(
    stream: I,
    app: axum::Router,
    options: ConnectionOptions,
    lifecycle: LifecycleSubscription,
) -> ConnectionReport
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    serve_io_with_deadline_source(stream, app, options, lifecycle, None).await
}

pub async fn serve_io_with_drain<I>(
    stream: I,
    app: axum::Router,
    options: ConnectionOptions,
    lifecycle: LifecycleSubscription,
    deadline: ShutdownDeadline,
) -> ConnectionReport
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    serve_io_with_deadline_source(stream, app, options, lifecycle, Some(deadline)).await
}

async fn serve_io_with_deadline_source<I>(
    stream: I,
    app: axum::Router,
    options: ConnectionOptions,
    mut lifecycle: LifecycleSubscription,
    fixed_deadline: Option<ShutdownDeadline>,
) -> ConnectionReport
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let accounting = Arc::new(Accounting::new());
    let service_lifecycle = Arc::new(Mutex::new(lifecycle.clone()));
    let (terminal, error) = {
        let mut builder = auto::Builder::new(TokioExecutor::new());
        builder
            .http2()
            .max_concurrent_streams(options.max_concurrent_streams());
        let io = TokioIo::new(stream);
        let shared = Arc::clone(&accounting);
        let service_lifecycle = Arc::clone(&service_lifecycle);
        let svc = service_fn(move |req: Request<hyper::body::Incoming>| {
            let app = app.clone();
            let shared = Arc::clone(&shared);
            let service_lifecycle = Arc::clone(&service_lifecycle);
            async move {
                let version = req.version();
                let method = req.method().clone();
                observe_drain(&shared, &service_lifecycle);
                let Some(guard) = shared.admit(version, &method) else {
                    let response = Response::builder()
                        .status(StatusCode::SERVICE_UNAVAILABLE)
                        .header("connection", "close")
                        .body(axum::body::Body::empty())
                        .expect("static response");
                    return Ok::<_, std::convert::Infallible>(response);
                };
                let mut response = app.oneshot(req).await;
                let draining = observe_drain(&shared, &service_lifecycle);
                if draining && version != Version::HTTP_2 {
                    if let Ok(response) = response.as_mut() {
                        response.headers_mut().insert(
                            http::header::CONNECTION,
                            http::HeaderValue::from_static("close"),
                        );
                    }
                }
                guard.complete();
                response
            }
        });
        let connection = builder.serve_connection_with_upgrades(io, svc);
        tokio::pin!(connection);
        loop {
            if lifecycle.observation().phase.is_draining_or_later() {
                accounting.begin_drain();
                connection.as_mut().graceful_shutdown();
                let Some(deadline) = fixed_deadline.or_else(|| lifecycle.shutdown_deadline())
                else {
                    break (
                        ConnectionTerminal::Failed,
                        Some("lifecycle entered draining without shutdown deadline".into()),
                    );
                };
                let usable = deadline.usable_remaining();
                if usable.is_zero() {
                    accounting.mark_deadline();
                    break (ConnectionTerminal::DeadlineExceeded, None);
                }
                match tokio::time::timeout(usable, &mut connection).await {
                    Ok(Ok(())) => break (ConnectionTerminal::Drained, None),
                    Ok(Err(error)) => break (ConnectionTerminal::Failed, Some(error.to_string())),
                    Err(_) => {
                        accounting.mark_deadline();
                        break (ConnectionTerminal::DeadlineExceeded, None);
                    }
                }
            }
            tokio::select! {
                result = &mut connection => break match result { Ok(()) => (if accounting.drain_started() { ConnectionTerminal::Drained } else { ConnectionTerminal::PeerClosed }, None), Err(error) => (ConnectionTerminal::Failed, Some(error.to_string())) },
                _ = lifecycle.changed() => {}
            }
        }
    };
    accounting.report(terminal, error)
}
