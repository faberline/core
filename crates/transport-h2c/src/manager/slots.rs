use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::Inner;
use crate::conn::ManagedConn;
use crate::error::{H2cError, Result};

/// Open one connection and track it in the pool, enforcing `max_connections`
/// via the `slots` reservation so no orphan (connected-but-untracked) socket is
/// ever created. Shared by on-demand growth and supervisor replenishment.
pub(super) async fn connect_tracked(inner: &Arc<Inner>) -> Result<Arc<ManagedConn>> {
    let cfg = &inner.cfg;
    // Claim a slot up front; back out if it would breach the cap.
    let prev = inner.slots.fetch_add(1, Ordering::AcqRel);
    if prev >= cfg.max_connections() {
        inner.slots.fetch_sub(1, Ordering::AcqRel);
        return Err(H2cError::NoConnection(format!(
            "{} at max_connections ({})",
            inner.authority,
            cfg.max_connections()
        )));
    }
    let id = inner.next_id.fetch_add(1, Ordering::Relaxed);
    let connect = ManagedConn::connect(id, &inner.authority, cfg.conn_config());
    let conn = match tokio::time::timeout(cfg.connect_timeout(), connect).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            inner.slots.fetch_sub(1, Ordering::AcqRel);
            return Err(e);
        }
        Err(_) => {
            inner.slots.fetch_sub(1, Ordering::AcqRel);
            return Err(H2cError::Timeout(cfg.connect_timeout()));
        }
    };
    inner.conns.write().await.push(conn.clone());
    Ok(conn)
}

/// Retire a connection's lifetime totals and free its slot (on evict / shrink).
pub(super) fn retire(inner: &Inner, c: &ManagedConn) {
    inner
        .retired_requests
        .fetch_add(c.total(), Ordering::Relaxed);
    inner
        .retired_errors
        .fetch_add(c.errors(), Ordering::Relaxed);
    inner.slots.fetch_sub(1, Ordering::AcqRel);
}
