use std::future::Future;
use std::sync::Arc;

use server_lifecycle::{ConnectionMetrics, LifecycleController};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use crate::config::TcpServerConfig;
use crate::connection::{ConnectionContext, TcpConnectionResult, TcpConnectionTerminal};
use crate::handler::TcpHandler;
use crate::report::TcpServerReport;

struct ConnectionCloseGuard(Arc<dyn ConnectionMetrics>);
impl Drop for ConnectionCloseGuard {
    fn drop(&mut self) {
        self.0.connection_closed();
    }
}

pub async fn bind(config: &TcpServerConfig) -> std::io::Result<TcpListener> {
    // @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#logic
    let addr = config.bind().socket_addr();
    let domain = if addr.is_ipv6() {
        Domain::IPV6
    } else {
        Domain::IPV4
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    if config.socket().reuse_addr() {
        socket.set_reuse_address(true)?;
    }
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(config.socket().backlog())?;
    TcpListener::from_std(std::net::TcpListener::from(socket))
}

pub async fn serve<H, S>(listener: TcpListener, config: TcpServerConfig, handler: H, shutdown: S)
where
    H: TcpHandler,
    S: Future<Output = ()> + Send + 'static,
{
    serve_arc(listener, config, Arc::new(handler), shutdown).await;
}

pub async fn serve_arc<H, S>(
    listener: TcpListener,
    config: TcpServerConfig,
    handler: Arc<H>,
    shutdown: S,
) where
    H: TcpHandler,
    S: Future<Output = ()> + Send + 'static,
{
    let handler_owner = Arc::clone(&handler);
    let report = serve_core(
        listener,
        config,
        move |stream, cx| {
            let handler = Arc::clone(&handler_owner);
            async move {
                match handler.handle(stream, cx).await {
                    Ok(()) => TcpConnectionResult::default(),
                    Err(error) => {
                        tracing::debug!(%error, "tcp connection handler failed");
                        TcpConnectionResult::new(TcpConnectionTerminal::Failed)
                    }
                }
            }
        },
        Box::pin(shutdown),
        None,
    )
    .await;
    let _ = report;
}

/// Report-capable production listener path. The supplied lifecycle is the
/// sole source of admission shutdown and its published absolute deadline.
/// Compatibility [`serve`] and [`serve_arc`] retain their legacy relative
/// timeout adapters.
async fn serve_core<H, F>(
    listener: TcpListener,
    config: TcpServerConfig,
    handler: H,
    shutdown: std::pin::Pin<Box<dyn Future<Output = ()> + Send>>,
    lifecycle: Option<LifecycleController>,
) -> TcpServerReport
where
    H: Fn(TcpStream, ConnectionContext) -> F + Send + Sync + 'static,
    F: Future<Output = TcpConnectionResult> + Send + 'static,
{
    let local_addr = listener.local_addr().ok();
    let handler = Arc::new(handler);
    let owner = Arc::clone(&handler);
    let lifecycle_mode = lifecycle.is_some();
    let lifecycle = lifecycle.unwrap_or_else(|| config.drain().lifecycle());
    let mut subscription = lifecycle.subscribe();
    let mut shutdown = shutdown;
    let mut tasks = JoinSet::new();
    let mut report = TcpServerReport::default();

    while !subscription.observation().phase.is_draining_or_later() {
        tokio::select! {
            joined = tasks.join_next(), if !tasks.is_empty() => {
                match joined {
                    Some(Ok(result)) => report.record(result),
                    Some(Err(_)) => report.failed += 1,
                    None => {}
                }
            }
            changed = subscription.changed() => {
                if changed.phase.is_draining_or_later() {
                    break;
                }
            }
            _ = &mut shutdown => {
                config.drain().start_drain();
                break;
            }
            accept = listener.accept() => {
                let (stream, peer_addr) = match accept {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::warn!(%error, "tcp accept failed");
                        report.accept_errors += 1;
                        continue;
                    }
                };
                if lifecycle.observation().phase.is_draining_or_later() {
                    config.connection_metrics().connection_rejected();
                    report.rejected += 1;
                    drop(stream);
                    break;
                }
                if let Err(error) = stream.set_nodelay(config.socket().nodelay()) {
                    tracing::debug!(%error, %peer_addr, "failed to set tcp nodelay");
                }
                let permit = match config.connection_budget() {
                    Some(budget) => match budget.try_acquire() {
                        Ok(permit) => Some(permit),
                        Err(error) => {
                            config.connection_metrics().connection_rejected();
                            tracing::warn!(%error, %peer_addr, "tcp connection rejected");
                            drop(stream);
                            report.rejected += 1;
                            continue;
                        }
                    },
                    None => None,
                };
                config.connection_metrics().connection_accepted();
                report.accepted += 1;
                let handler = Arc::clone(&handler);
                let connection_metrics = Arc::clone(config.connection_metrics());
                let closed = ConnectionCloseGuard(connection_metrics);
                let cx = ConnectionContext {
                    local_addr: local_addr.unwrap_or_else(|| stream.local_addr().unwrap_or(config.bind().socket_addr())),
                    peer_addr,
                    drain: config.drain().signal(),
                    lifecycle: lifecycle.subscribe(),
                };
                tasks.spawn(async move {
                    let _permit = permit;
                    let _closed = closed;
                    handler(stream, cx).await
                });
            }
        }
    }

    drop(listener);
    let remaining = if lifecycle_mode {
        match lifecycle.subscribe().shutdown_deadline() {
            Some(deadline) => deadline.remaining(),
            None => {
                report.deadline_missing = true;
                report.unfinished += tasks.len() as u64;
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                drop(owner);
                return report;
            }
        }
    } else {
        config.drain_timeout()
    };
    if remaining.is_zero() {
        report.unfinished += tasks.len() as u64;
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    } else {
        let join = async {
            while let Some(joined) = tasks.join_next().await {
                match joined {
                    Ok(result) => report.record(result),
                    Err(_) => report.failed += 1,
                }
            }
        };
        if tokio::time::timeout(remaining, join).await.is_err() {
            report.unfinished += tasks.len() as u64;
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        }
    }
    drop(owner);
    report
}

pub async fn serve_with_report<H, F>(
    listener: TcpListener,
    config: TcpServerConfig,
    handler: H,
    lifecycle: LifecycleController,
) -> TcpServerReport
where
    H: Fn(TcpStream, ConnectionContext) -> F + Send + Sync + 'static,
    F: Future<Output = TcpConnectionResult> + Send + 'static,
{
    let mut subscription = lifecycle.subscribe();
    let shutdown = async move {
        if subscription.observation().phase.is_draining_or_later() {
            return;
        }
        loop {
            let observation = subscription.changed().await;
            if observation.phase.is_draining_or_later() {
                break;
            }
        }
    };
    serve_core(
        listener,
        config,
        handler,
        Box::pin(shutdown),
        Some(lifecycle),
    )
    .await
}

#[cfg(test)]
mod tests;
