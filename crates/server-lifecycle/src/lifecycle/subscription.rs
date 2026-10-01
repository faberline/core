use tokio::sync::{broadcast, watch};

use super::error::{LifecycleDeadlineError, LifecycleEventError, LifecycleSubscriptionError};
use super::observation::LifecycleObservation;
use crate::deadline::ShutdownDeadline;

#[derive(Debug, Clone)]
pub struct LifecycleSubscription {
    pub(crate) rx: watch::Receiver<LifecycleObservation>,
    pub(crate) deadline_rx: watch::Receiver<Option<ShutdownDeadline>>,
}

impl LifecycleSubscription {
    pub fn observation(&self) -> LifecycleObservation {
        self.rx.borrow().clone()
    }

    pub async fn changed(&mut self) -> LifecycleObservation {
        let _ = self.rx.changed().await;
        self.observation()
    }

    pub async fn changed_result(
        &mut self,
    ) -> Result<LifecycleObservation, LifecycleSubscriptionError> {
        self.rx
            .changed()
            .await
            .map_err(|_| LifecycleSubscriptionError::Closed)?;
        Ok(self.observation())
    }

    /// The authoritative absolute deadline published by the first shutdown attempt.
    pub fn shutdown_deadline(&self) -> Option<ShutdownDeadline> {
        *self.deadline_rx.borrow()
    }

    /// Wait for the first shutdown deadline, or report that its publication
    /// channel was closed before a deadline became available.
    pub async fn wait_shutdown_deadline(
        &mut self,
    ) -> Result<ShutdownDeadline, LifecycleDeadlineError> {
        loop {
            if let Some(deadline) = self.shutdown_deadline() {
                return Ok(deadline);
            }
            if self.deadline_rx.changed().await.is_err() {
                return Err(LifecycleDeadlineError::ChannelClosed);
            }
        }
    }

    /// Compatibility spelling for callers that prefer the explicit `for` form.
    pub async fn wait_for_shutdown_deadline(
        &mut self,
    ) -> Result<ShutdownDeadline, LifecycleDeadlineError> {
        self.wait_shutdown_deadline().await
    }
}

pub struct LifecycleEventSubscription {
    pub(super) initial: Option<LifecycleObservation>,
    pub(super) rx: broadcast::Receiver<LifecycleObservation>,
}

impl LifecycleEventSubscription {
    pub async fn next(&mut self) -> Result<LifecycleObservation, LifecycleEventError> {
        if let Some(initial) = self.initial.take() {
            return Ok(initial);
        }
        self.rx.recv().await.map_err(|error| match error {
            broadcast::error::RecvError::Lagged(n) => LifecycleEventError::Lagged(n),
            broadcast::error::RecvError::Closed => LifecycleEventError::Closed,
        })
    }
}
