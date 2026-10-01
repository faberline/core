pub use server_lifecycle::{shutdown_with_drain, wait_shutdown_signal};
use server_lifecycle::{LifecycleController, ShutdownDeadline, ShutdownReport};
use std::{sync::Arc, time::Duration};

#[derive(Clone)]
pub struct LifecycleShutdownTrigger {
    lifecycle: LifecycleController,
    total: Duration,
    reserve: Duration,
}

impl LifecycleShutdownTrigger {
    pub fn new(
        lifecycle: LifecycleController,
        total: Duration,
        reserve: Duration,
    ) -> Result<Self, server_lifecycle::DeadlineError> {
        ShutdownDeadline::from_now(total, reserve)?;
        Ok(Self {
            lifecycle,
            total,
            reserve,
        })
    }

    pub async fn trigger(
        &self,
        reason_code: impl Into<String>,
        detail: impl Into<String>,
    ) -> Arc<ShutdownReport> {
        let deadline = ShutdownDeadline::from_now(self.total, self.reserve)
            .expect("validated shutdown durations");
        self.lifecycle.shutdown(deadline, reason_code, detail).await
    }
}

pub async fn run_signal_bridge<F>(
    trigger: LifecycleShutdownTrigger,
    signal: F,
) -> Arc<ShutdownReport>
where
    F: std::future::Future<Output = ()> + Send,
{
    signal.await;
    trigger.trigger("signal", "shutdown signal received").await
}

/// POSIX/CTRL-C convenience bridge for production binaries.
pub async fn shutdown_on_signal(trigger: LifecycleShutdownTrigger) -> Arc<ShutdownReport> {
    run_signal_bridge(trigger, wait_shutdown_signal()).await
}
