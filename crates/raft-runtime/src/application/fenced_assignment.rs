//! Deterministic committed executor ownership and fencing.
//!
//! [`FencedAssignment`] is intended to live inside a service-owned
//! [`crate::RaftStateMachine`]. The application still owns the assignment key
//! (message, task, queue, shard, ...), command encoding, and domain outcome;
//! this module only supplies the transition invariants shared by effectful
//! services. Calling these methods outside committed state-machine apply does
//! not make an assignment authoritative.

use std::error::Error;
use std::fmt;

use raft_core::NodeId;
use serde::{Deserialize, Serialize};

/// Monotonic token identifying one committed ownership generation.
///
/// Build one with [`AssignmentEpoch::new`] and read the number back with
/// [`AssignmentEpoch::get`]. It serializes as the bare number, and `{:?}` /
/// `{}` print the bare number. `AssignmentEpoch::default()` (0) is the idle
/// epoch before the first assignment.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssignmentEpoch(u64);

impl AssignmentEpoch {
    /// The epoch with this number.
    pub const fn new(epoch: u64) -> Self {
        Self(epoch)
    }

    /// The bare number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The epoch after this one, or `None` when the number space is used up.
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(epoch) => Some(Self(epoch)),
            None => None,
        }
    }
}

impl fmt::Debug for AssignmentEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for AssignmentEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// Proof that one replica owns an assignment at one fencing epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FenceToken {
    pub owner: NodeId,
    pub epoch: AssignmentEpoch,
}

/// The currently committed, time-bounded assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveAssignment {
    pub token: FenceToken,
    /// Absolute timestamp chosen by the proposer before the command enters
    /// Raft. State-machine apply must never read a replica-local clock.
    pub expires_at_ms: u64,
}

/// Durable fencing state for one application-owned assignment key.
///
/// Expiry is deliberately an explicit transition: an elapsed wall clock does
/// not silently clear ownership on one replica. The application proposes an
/// expire/reclaim command containing `now_ms`, applies [`expire`](Self::expire)
/// on every replica, and may assign a new owner only after that commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FencedAssignment {
    epoch: AssignmentEpoch,
    active: Option<ActiveAssignment>,
}

impl FencedAssignment {
    /// Empty ownership state. The first assignment receives epoch 1.
    pub const fn idle() -> Self {
        Self {
            epoch: AssignmentEpoch::new(0),
            active: None,
        }
    }

    /// Highest epoch ever assigned, retained while idle so epochs never repeat.
    pub const fn epoch(&self) -> AssignmentEpoch {
        self.epoch
    }

    /// The current committed assignment, including one that has passed its
    /// deadline but has not yet been explicitly expired by a committed command.
    pub const fn active(&self) -> Option<ActiveAssignment> {
        self.active
    }

    /// A token is available only after assignment commit. Executor adapters use
    /// this as their commit-before-effect gate.
    pub const fn token(&self) -> Option<FenceToken> {
        match self.active {
            Some(active) => Some(active.token),
            None => None,
        }
    }

    /// Assign an idle key to `owner`, incrementing the fencing epoch.
    pub fn assign(
        &mut self,
        owner: NodeId,
        now_ms: u64,
        expires_at_ms: u64,
    ) -> Result<FenceToken, AssignmentError> {
        if expires_at_ms <= now_ms {
            return Err(AssignmentError::InvalidExpiry {
                now_ms,
                expires_at_ms,
            });
        }
        if let Some(active) = self.active {
            return Err(AssignmentError::AlreadyAssigned(active));
        }
        let epoch = self
            .epoch
            .checked_next()
            .ok_or(AssignmentError::EpochExhausted)?;
        let token = FenceToken { owner, epoch };
        self.epoch = epoch;
        self.active = Some(ActiveAssignment {
            token,
            expires_at_ms,
        });
        Ok(token)
    }

    /// Validate that `token` is the current unexpired owner at `now_ms`.
    pub fn validate(
        &self,
        token: FenceToken,
        now_ms: u64,
    ) -> Result<ActiveAssignment, AssignmentError> {
        let active = self.active.ok_or(AssignmentError::Unassigned {
            current_epoch: self.epoch,
        })?;
        if token.epoch != active.token.epoch {
            return Err(AssignmentError::StaleEpoch {
                current: active.token.epoch,
                provided: token.epoch,
            });
        }
        if token.owner != active.token.owner {
            return Err(AssignmentError::OwnerMismatch {
                current: active.token.owner,
                provided: token.owner,
            });
        }
        if now_ms >= active.expires_at_ms {
            return Err(AssignmentError::Expired {
                expires_at_ms: active.expires_at_ms,
                now_ms,
            });
        }
        Ok(active)
    }

    /// Extend an unexpired assignment. Renewals cannot shorten the deadline.
    pub fn renew(
        &mut self,
        token: FenceToken,
        now_ms: u64,
        expires_at_ms: u64,
    ) -> Result<ActiveAssignment, AssignmentError> {
        let active = self.validate(token, now_ms)?;
        if expires_at_ms <= active.expires_at_ms {
            return Err(AssignmentError::ExpiryNotExtended {
                current: active.expires_at_ms,
                proposed: expires_at_ms,
            });
        }
        let renewed = ActiveAssignment {
            token,
            expires_at_ms,
        };
        self.active = Some(renewed);
        Ok(renewed)
    }

    /// Complete, cancel, or voluntarily release an unexpired assignment.
    /// The epoch is retained and the next assignment receives a higher token.
    pub fn release(
        &mut self,
        token: FenceToken,
        now_ms: u64,
    ) -> Result<ActiveAssignment, AssignmentError> {
        let active = self.validate(token, now_ms)?;
        self.active = None;
        Ok(active)
    }

    /// Apply the committed expiry/reclaim transition after the deadline.
    pub fn expire(&mut self, now_ms: u64) -> Result<ActiveAssignment, AssignmentError> {
        let active = self.active.ok_or(AssignmentError::Unassigned {
            current_epoch: self.epoch,
        })?;
        if now_ms < active.expires_at_ms {
            return Err(AssignmentError::NotExpired {
                expires_at_ms: active.expires_at_ms,
                now_ms,
            });
        }
        self.active = None;
        Ok(active)
    }
}

/// Deterministic transition rejection returned identically by every replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentError {
    InvalidExpiry {
        now_ms: u64,
        expires_at_ms: u64,
    },
    AlreadyAssigned(ActiveAssignment),
    Unassigned {
        current_epoch: AssignmentEpoch,
    },
    StaleEpoch {
        current: AssignmentEpoch,
        provided: AssignmentEpoch,
    },
    OwnerMismatch {
        current: NodeId,
        provided: NodeId,
    },
    Expired {
        expires_at_ms: u64,
        now_ms: u64,
    },
    NotExpired {
        expires_at_ms: u64,
        now_ms: u64,
    },
    ExpiryNotExtended {
        current: u64,
        proposed: u64,
    },
    EpochExhausted,
}

impl fmt::Display for AssignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExpiry {
                now_ms,
                expires_at_ms,
            } => write!(
                f,
                "assignment expiry {expires_at_ms} must be after {now_ms}"
            ),
            Self::AlreadyAssigned(active) => write!(
                f,
                "assignment is owned by node {} at epoch {} until {}",
                active.token.owner, active.token.epoch, active.expires_at_ms
            ),
            Self::Unassigned { current_epoch } => {
                write!(f, "assignment is idle at epoch {current_epoch}")
            }
            Self::StaleEpoch { current, provided } => {
                write!(
                    f,
                    "stale assignment epoch {provided}; current epoch is {current}"
                )
            }
            Self::OwnerMismatch { current, provided } => {
                write!(
                    f,
                    "assignment owner {provided} does not match current owner {current}"
                )
            }
            Self::Expired {
                expires_at_ms,
                now_ms,
            } => write!(
                f,
                "assignment expired at {expires_at_ms}; current time is {now_ms}"
            ),
            Self::NotExpired {
                expires_at_ms,
                now_ms,
            } => write!(
                f,
                "assignment expires at {expires_at_ms}; current time is {now_ms}"
            ),
            Self::ExpiryNotExtended { current, proposed } => write!(
                f,
                "renewal expiry {proposed} must be greater than current expiry {current}"
            ),
            Self::EpochExhausted => f.write_str("assignment epoch exhausted"),
        }
    }
}

impl Error for AssignmentError {}

#[cfg(test)]
mod tests;
