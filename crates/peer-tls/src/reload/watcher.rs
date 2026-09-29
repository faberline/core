use std::time::Duration;

use super::ReloadableTls;

/// Poll cadence for a projected Secret. kubelet refreshes a projected volume on
/// its own schedule, so a short poll buys nothing; a leaf renewed hours before
/// expiry has no deadline this misses.
pub const DEFAULT_MATERIAL_POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Poll the source and activate whatever validates, forever.
///
/// A rejected candidate is counted and logged, never fatal: the material on disk
/// during a two-step Secret update is briefly inconsistent by construction, and
/// a watcher that gave up on the first bad read would turn every rotation into
/// an outage.
pub fn spawn_material_watcher(
    tls: ReloadableTls,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    let interval = if interval.is_zero() {
        Duration::from_secs(1)
    } else {
        interval
    };
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(rejection) = tls.reload() {
                tracing::warn!(
                    target: "peer_tls.reload",
                    reason = rejection.reason.as_str(),
                    detail = %rejection.detail,
                    "projected TLS material rejected; retaining the last valid generation"
                );
            }
        }
    })
}
