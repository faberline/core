use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use tokio::time::MissedTickBehavior;

use super::slots::{connect_tracked, retire};
use super::Inner;
use crate::conn::ManagedConn;

/// Background supervisor: ping for liveness, evict dead, shrink idle, replenish
/// to `min_connections`. Exits when the manager is fully dropped or shut down.
pub(super) async fn supervise(weak: Weak<Inner>) {
    let interval = match weak.upgrade() {
        Some(inner) => inner.cfg.ping_interval(),
        None => return,
    };
    let mut tick = tokio::time::interval(interval);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    tick.tick().await; // consume the immediate first fire

    loop {
        tick.tick().await;
        let Some(inner) = weak.upgrade() else { break };
        if inner.shutdown.load(Ordering::Acquire) {
            break;
        }

        // 1. Liveness-ping healthy connections; flag the dead ones.
        let snapshot: Vec<Arc<ManagedConn>> = inner.conns.read().await.clone();
        for c in &snapshot {
            if c.is_healthy() {
                if let Err(e) = c.ping().await {
                    tracing::debug!(conn = c.id, error = %e, "liveness ping failed; evicting");
                    c.mark_dead();
                }
            }
        }

        // 2. Evict dead + shrink one idle connection above the minimum.
        {
            let mut conns = inner.conns.write().await;
            let min = inner.cfg.min_connections();
            let max_idle = inner.cfg.max_keepalive_connections().max(min);
            conns.retain(|c| {
                let keep = c.is_healthy();
                if !keep {
                    retire(&inner, c);
                }
                keep
            });
            if conns.len() > min {
                if let Some(pos) = conns.iter().position(|c| {
                    c.in_flight() == 0
                        && (conns.len() > max_idle || c.idle() >= inner.cfg.idle_timeout())
                }) {
                    let c = conns.remove(pos);
                    retire(&inner, &c);
                    tracing::debug!(conn = c.id, "shrinking idle h2c connection");
                }
            }
        }

        // 3. Replenish to the warm minimum.
        let deficit = inner
            .cfg
            .min_connections()
            .saturating_sub(inner.conns.read().await.len());
        for _ in 0..deficit {
            if let Err(e) = connect_tracked(&inner).await {
                tracing::debug!(error = %e, "replenish connect failed");
                break;
            }
        }
        // `inner` dropped here, before the next idle tick, so the Weak can die.
    }
}
