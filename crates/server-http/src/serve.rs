use std::sync::Arc;

use server_lifecycle::{BindConfig, DrainController, LifecycleController};
use tokio::net::TcpListener;

use crate::options::HttpServerOptions;

/// Connection and request-stream totals of one lifecycle-driven HTTP run.
///
/// The HTTP listener reports exactly what server-tcp's accept loop counts, so
/// this is the same type as [`server_tcp::TcpServerReport`].
pub type HttpServerReport = server_tcp::TcpServerReport;

/// Production HTTP composition. The supplied lifecycle owns listener drain,
/// per-connection subscriptions, and the shutdown-time absolute deadline.
pub async fn serve_h2c_with_lifecycle(
    listener: TcpListener,
    app: axum::Router,
    options: HttpServerOptions,
    lifecycle: LifecycleController,
) -> HttpServerReport {
    let local_addr = listener
        .local_addr()
        .unwrap_or_else(|_| BindConfig::default().socket_addr());
    let mut tcp_config = server_tcp::TcpServerConfig::new(BindConfig::from(local_addr))
        .with_socket_options(options.socket())
        .with_drain(DrainController::from_lifecycle(lifecycle.clone()))
        .with_connection_metrics(Arc::clone(options.connection_metrics()));
    if let Some(budget) = options.connection_budget() {
        tcp_config = tcp_config.with_connection_budget(budget.clone());
    }
    let connection_options =
        transport_h2c::ConnectionOptions::new(options.max_concurrent_streams());
    server_tcp::serve_with_report(
        listener,
        tcp_config,
        move |stream, cx: server_tcp::ConnectionContext| {
            let app = app.clone();
            async move {
                let connection = transport_h2c::serve_connection_with_lifecycle(
                    stream,
                    app,
                    connection_options,
                    cx.lifecycle,
                )
                .await;
                let terminal = match connection.terminal {
                    transport_h2c::ConnectionTerminal::DeadlineExceeded => {
                        server_tcp::TcpConnectionTerminal::TimedOut
                    }
                    transport_h2c::ConnectionTerminal::Failed => {
                        server_tcp::TcpConnectionTerminal::Failed
                    }
                    transport_h2c::ConnectionTerminal::PeerClosed
                    | transport_h2c::ConnectionTerminal::Drained => {
                        server_tcp::TcpConnectionTerminal::Completed
                    }
                };
                server_tcp::TcpConnectionResult::new(terminal)
                    .with_streams_admitted(connection.admitted as u64)
                    .with_streams_active_at_drain(connection.active_at_drain as u64)
                    .with_streams_completed(connection.completed as u64)
                    .with_streams_refused(connection.refused as u64)
                    .with_streams_timed_out(connection.timed_out as u64)
                    .with_streams_ambiguous(connection.ambiguous as u64)
            }
        },
        lifecycle,
    )
    .await
}

/// Serve HTTP/1.1 + h2c on one listener.
///
/// Legacy/test-dev adapter; production composition should use
/// [`serve_h2c_with_lifecycle`] so one caller-supplied lifecycle owns drain.
/// @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#logic
pub async fn serve_h2c(
    listener: TcpListener,
    app: axum::Router,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) {
    serve_h2c_with_options(listener, app, HttpServerOptions::default(), shutdown).await;
}

/// Serve HTTP/1.1 + h2c with tunable HTTP/2 stream and drain settings.
///
/// Legacy/test-dev adapter retained for source compatibility. Production
/// callers should use [`serve_h2c_with_lifecycle`].
pub async fn serve_h2c_with_options(
    listener: TcpListener,
    app: axum::Router,
    options: HttpServerOptions,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) {
    let local_addr = listener
        .local_addr()
        .unwrap_or_else(|_| BindConfig::default().socket_addr());
    let mut tcp_config = server_tcp::TcpServerConfig::new(BindConfig::from(local_addr))
        .with_socket_options(options.socket())
        .with_drain(options.drain().clone())
        .with_drain_timeout(options.drain_timeout())
        .with_connection_metrics(Arc::clone(options.connection_metrics()));
    if let Some(budget) = options.connection_budget() {
        tcp_config = tcp_config.with_connection_budget(budget.clone());
    }

    let connection_options =
        transport_h2c::ConnectionOptions::new(options.max_concurrent_streams());
    server_tcp::serve(
        listener,
        tcp_config,
        move |stream, cx| {
            let app = app.clone();
            async move {
                let _ = cx;
                transport_h2c::serve_connection_with_options(stream, app, connection_options)
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))
            }
        },
        shutdown,
    )
    .await;
}

#[cfg(test)]
mod tests;
