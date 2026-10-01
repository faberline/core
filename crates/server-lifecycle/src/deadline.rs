use std::time::Duration;

use tokio::time::Instant;

/// One absolute shutdown budget shared by every lifecycle participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownDeadline {
    expires_at: Instant,
    total: Duration,
    reserve: Duration,
}

impl ShutdownDeadline {
    /// A deadline that expires at `expires_at` for a budget of `total`, of
    /// which `reserve` is held back for the final step. Fails when the
    /// reserve exceeds the total.
    pub fn new(
        expires_at: Instant,
        total: Duration,
        reserve: Duration,
    ) -> Result<Self, DeadlineError> {
        if reserve > total {
            return Err(DeadlineError::ReserveExceedsTotal { total, reserve });
        }
        Ok(Self {
            expires_at,
            total,
            reserve,
        })
    }

    /// A deadline that expires `total` from now. Fails when the reserve
    /// exceeds the total.
    pub fn from_now(total: Duration, reserve: Duration) -> Result<Self, DeadlineError> {
        Self::new(Instant::now() + total, total, reserve)
    }

    /// The instant the whole budget runs out.
    pub fn expires_at(&self) -> Instant {
        self.expires_at
    }

    /// The whole budget, reserve included.
    pub fn total(&self) -> Duration {
        self.total
    }

    /// The part of the budget held back for the final step.
    pub fn reserve(&self) -> Duration {
        self.reserve
    }

    pub fn remaining(self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    pub fn usable_remaining(self) -> Duration {
        self.remaining().saturating_sub(self.reserve)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DeadlineError {
    #[error("shutdown reserve {reserve:?} exceeds total {total:?}")]
    ReserveExceedsTotal { total: Duration, reserve: Duration },
}
