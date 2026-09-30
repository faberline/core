use server_lifecycle::LifecycleController;
use tokio::net::TcpListener;
use tower_http::trace::{DefaultMakeSpan, MakeSpan};

/// Request span maker that preserves standard request fields and, in an
/// OTLP-enabled build, attaches a valid propagated W3C parent context.
#[derive(Debug, Clone, Copy)]
pub struct PropagatingMakeSpan;

impl<B> MakeSpan<B> for PropagatingMakeSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        let mut default = DefaultMakeSpan::new().level(tracing::Level::INFO);
        let span = default.make_span(request);
        #[cfg(feature = "otlp")]
        {
            use opentelemetry::trace::TraceContextExt as _;
            use tracing_opentelemetry::OpenTelemetrySpanExt as _;
            let parent = service_observability::extract_trace_context(request.headers());
            if parent.span().span_context().is_valid() {
                span.set_parent(parent);
            }
        }
        span
    }
}

/// Serve `app` (HTTP/1.1 + h2c on one port) on `listener`, stopping when
/// `shutdown` resolves (e.g. [`crate::shutdown_with_drain`]).
///
/// Thin delegation to [`server_http::serve_h2c`] — the shared HTTP runtime — so
/// a service does not hand-roll the hyper-util auto-builder accept loop.
/// In-flight connections
/// get a bounded grace period after `shutdown` resolves before the process
/// exits.
pub async fn serve(
    listener: TcpListener,
    app: axum::Router,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) {
    // Legacy adapter: production callers should use `serve_with_lifecycle` so
    // listener admission and shutdown deadline come from one controller.
    server_http::serve_h2c(listener, app, shutdown).await;
}

/// Production transport composition over the caller-owned lifecycle.
pub async fn serve_with_lifecycle(
    listener: TcpListener,
    app: axum::Router,
    options: server_http::HttpServerOptions,
    lifecycle: LifecycleController,
) -> server_http::HttpServerReport {
    server_http::serve_h2c_with_lifecycle(listener, app, options, lifecycle).await
}

/// Serve `app` over TLS on `listener`, terminating with whatever `config`
/// returns at the moment each connection is accepted (#3113 R1).
///
/// The same thin delegation as [`serve`], to the same drain semantics, over
/// [`server_http::serve_tls`]. It is a separate entry point rather than an
/// option on `serve` because the two differ in exactly one way that matters:
/// this one has no cleartext branch. A service that fails to supply material
/// refuses connections; it does not answer them unencrypted.
///
/// ALPN comes from the `ServerConfig` the source yields, so the caller decides
/// whether the port offers `h2` alone or `h2` and `http/1.1`.
pub async fn serve_tls(
    listener: TcpListener,
    app: axum::Router,
    config: server_http::ServerConfigSource,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) {
    server_http::serve_tls(
        listener,
        app,
        config,
        server_http::TlsServerOptions::default(),
        shutdown,
    )
    .await;
}

#[cfg(test)]
mod delegation_tests {
    use super::*;
    use axum::{routing::get, Router};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn serve_delegates_listener_to_shared_http_runtime() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let app = Router::new().route("/delegated", get(|| async { "shared-runtime" }));

        let server = tokio::spawn(serve(listener, app, async move {
            let _ = shutdown_rx.await;
        }));

        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("connect delegated runtime");
        stream
            .write_all(b"GET /delegated HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .expect("write delegated request");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read delegated response");
        let response = String::from_utf8_lossy(&response);
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "service shell must expose the shared runtime listener: {response}"
        );
        assert!(
            response.ends_with("shared-runtime"),
            "service shell must preserve router behavior through delegation: {response}"
        );

        let _ = shutdown_tx.send(());
        tokio::time::timeout(std::time::Duration::from_secs(3), server)
            .await
            .expect("delegated server shutdown")
            .expect("delegated server task");
    }
}
