#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecyclePhase {
    Starting,
    Recovering,
    Serving,
    Degraded,
    Draining,
    Stopping,
    Stopped,
    Fatal,
}

impl LifecyclePhase {
    pub fn is_draining_or_later(self) -> bool {
        matches!(
            self,
            Self::Draining | Self::Stopping | Self::Stopped | Self::Fatal
        )
    }
}

pub(super) fn valid_edge(from: LifecyclePhase, to: LifecyclePhase) -> bool {
    matches!(
        (from, to),
        (
            LifecyclePhase::Starting,
            LifecyclePhase::Recovering
                | LifecyclePhase::Serving
                | LifecyclePhase::Degraded
                | LifecyclePhase::Draining
                | LifecyclePhase::Fatal
        ) | (
            LifecyclePhase::Recovering,
            LifecyclePhase::Serving
                | LifecyclePhase::Degraded
                | LifecyclePhase::Draining
                | LifecyclePhase::Fatal
        ) | (
            LifecyclePhase::Serving,
            LifecyclePhase::Degraded | LifecyclePhase::Draining | LifecyclePhase::Fatal
        ) | (
            LifecyclePhase::Degraded,
            LifecyclePhase::Serving | LifecyclePhase::Draining | LifecyclePhase::Fatal
        ) | (
            LifecyclePhase::Draining,
            LifecyclePhase::Stopping | LifecyclePhase::Fatal
        ) | (
            LifecyclePhase::Stopping,
            LifecyclePhase::Stopped | LifecyclePhase::Fatal
        )
    )
}
