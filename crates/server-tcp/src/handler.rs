use std::future::Future;

use anyhow::Result;
use tokio::net::TcpStream;

use crate::connection::ConnectionContext;

/// Zero-boxing TCP protocol handler.
/// @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#logic
///
/// A blanket impl for closures keeps call sites terse while avoiding the
/// boxed-future cost of `async_trait` on every accepted connection.
pub trait TcpHandler: Send + Sync + 'static {
    type Future: Future<Output = Result<()>> + Send + 'static;

    fn handle(&self, stream: TcpStream, cx: ConnectionContext) -> Self::Future;
}

impl<F, Fut> TcpHandler for F
where
    F: Fn(TcpStream, ConnectionContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    type Future = Fut;

    fn handle(&self, stream: TcpStream, cx: ConnectionContext) -> Self::Future {
        self(stream, cx)
    }
}
