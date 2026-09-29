use super::phase::LifecyclePhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("invalid lifecycle transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: LifecyclePhase,
        to: LifecyclePhase,
    },
    #[error("lifecycle hook registration is closed")]
    RegistrationClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleEventError {
    #[error("lifecycle event stream lagged by {0} transitions")]
    Lagged(u64),
    #[error("lifecycle event stream closed")]
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleSubscriptionError {
    #[error("lifecycle subscription closed")]
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleDeadlineError {
    #[error("lifecycle shutdown deadline publication channel closed")]
    ChannelClosed,
}
