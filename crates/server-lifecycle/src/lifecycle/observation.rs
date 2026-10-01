use tokio::time::Instant;

use super::phase::LifecyclePhase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleObservation {
    pub phase: LifecyclePhase,
    pub generation: u64,
    pub transitioned_at: Instant,
    pub reason_code: String,
    pub detail: String,
    /// Whether new work may be admitted in the current phase.  This is only
    /// meaningful for `Serving` and `Degraded`; all other phases are closed.
    pub admission_open: bool,
}

impl LifecycleObservation {
    pub fn startup_succeeded(&self) -> bool {
        matches!(
            self.phase,
            LifecyclePhase::Serving
                | LifecyclePhase::Degraded
                | LifecyclePhase::Draining
                | LifecyclePhase::Stopping
                | LifecyclePhase::Stopped
        )
    }

    pub fn is_healthy(&self) -> bool {
        matches!(
            self.phase,
            LifecyclePhase::Starting
                | LifecyclePhase::Recovering
                | LifecyclePhase::Serving
                | LifecyclePhase::Degraded
                | LifecyclePhase::Draining
                | LifecyclePhase::Stopping
        )
    }

    pub fn is_ready(&self) -> bool {
        self.admission_open
    }
}
