//! Pure decision core for StatefulSet rolling update partition progression.
//!
//! A StatefulSet with `spec.updateStrategy.rollingUpdate.partition = P` updates
//! only pods with ordinal in `[P, N)`. Pods `< P` stay on the old revision.
//!
//! [`plan_rollout`] computes the single next partition step from observed
//! StatefulSet facts and application-supplied health facts, progressing from
//! `P = N` down to `P = 0`, one member at a time. It owns partition and ordinal
//! selection, generation gating, pause and resume, and recovery from an
//! interrupted reconcile.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Member ordinal within a StatefulSet (`0..replicas`).
pub type Ordinal = u32;

/// Observed status of one StatefulSet member pod.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedMember {
    pub revision: String,
    pub ready: bool,
}

impl ObservedMember {
    pub fn new(revision: impl Into<String>, ready: bool) -> Self {
        Self {
            revision: revision.into(),
            ready,
        }
    }
}

/// Observed StatefulSet facts required to plan the next rollout step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedRollout {
    pub replicas: u32,
    pub generation: i64,
    pub observed_generation: i64,
    pub current_revision: String,
    pub update_revision: String,
    pub partition: u32,
    pub members: BTreeMap<Ordinal, ObservedMember>,
}

impl ObservedRollout {
    pub fn new(
        replicas: u32,
        generation: i64,
        observed_generation: i64,
        current_revision: impl Into<String>,
        update_revision: impl Into<String>,
        partition: u32,
    ) -> Self {
        Self {
            replicas,
            generation,
            observed_generation,
            current_revision: current_revision.into(),
            update_revision: update_revision.into(),
            partition,
            members: BTreeMap::new(),
        }
    }

    pub fn with_member(
        mut self,
        ordinal: Ordinal,
        revision: impl Into<String>,
        ready: bool,
    ) -> Self {
        self.members
            .insert(ordinal, ObservedMember::new(revision, ready));
        self
    }
}

/// External gates controlling rollout progression.
///
/// Application-layer health facts are supplied per ordinal. A missing ordinal
/// counts as unhealthy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutGate {
    pub paused: bool,
    pub app_healthy: BTreeMap<Ordinal, bool>,
}

impl RolloutGate {
    pub fn new(paused: bool, app_healthy: BTreeMap<Ordinal, bool>) -> Self {
        Self {
            paused,
            app_healthy,
        }
    }

    pub fn unpaused(app_healthy: BTreeMap<Ordinal, bool>) -> Self {
        Self {
            paused: false,
            app_healthy,
        }
    }

    pub fn paused() -> Self {
        Self {
            paused: true,
            app_healthy: BTreeMap::new(),
        }
    }
}

/// Reason preventing partition progression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoldReason {
    /// Rollout is explicitly paused.
    Paused,
    /// Controller has not yet observed the desired StatefulSet generation.
    GenerationMismatch,
    /// The released member has not yet updated to the desired revision.
    MemberNotUpdated,
    /// The released member has updated but is not yet ready.
    MemberNotReady,
    /// The released member is ready but application health facts report unhealthy.
    MemberUnhealthy,
    /// More than one released member is still on an old revision.
    MultipleMembersInFlight,
}

/// Next decision produced by the rollout planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RolloutStep {
    /// Progress is held until conditions are met.
    Hold { reason: HoldReason },
    /// Lower the partition to release the specified member.
    SetPartition { partition: u32, member: Ordinal },
    /// Every member is on update_revision, ready, and healthy.
    Complete,
}

/// Decide the next partition step from observed facts and app health.
pub fn plan_rollout(observed: &ObservedRollout, gate: &RolloutGate) -> RolloutStep {
    if gate.paused {
        return RolloutStep::Hold {
            reason: HoldReason::Paused,
        };
    }

    // Check for multiple members in flight:
    // If the observed partition is below the lowest ordinal still on an old
    // revision plus one (more than one member in flight), hold.
    let in_flight_count = (observed.partition..observed.replicas)
        .filter(|&ordinal| {
            observed
                .members
                .get(&ordinal)
                .map(|m| m.revision != observed.update_revision)
                .unwrap_or(true)
        })
        .count();

    if in_flight_count > 1 {
        return RolloutStep::Hold {
            reason: HoldReason::MultipleMembersInFlight,
        };
    }

    // Generation gating:
    // The next member is released only when observed_generation == generation.
    if observed.observed_generation != observed.generation {
        return RolloutStep::Hold {
            reason: HoldReason::GenerationMismatch,
        };
    }

    // Check complete condition:
    // Complete only when partition == 0, every member is on update_revision,
    // and every member is ready and healthy.
    if observed.partition == 0 {
        for ordinal in 0..observed.replicas {
            let member = observed.members.get(&ordinal);
            let runs_update = member
                .map(|m| m.revision == observed.update_revision)
                .unwrap_or(false);
            if !runs_update {
                return RolloutStep::Hold {
                    reason: HoldReason::MemberNotUpdated,
                };
            }
            let is_ready = member.map(|m| m.ready).unwrap_or(false);
            if !is_ready {
                return RolloutStep::Hold {
                    reason: HoldReason::MemberNotReady,
                };
            }
            let is_healthy = gate.app_healthy.get(&ordinal).copied().unwrap_or(false);
            if !is_healthy {
                return RolloutStep::Hold {
                    reason: HoldReason::MemberUnhealthy,
                };
            }
        }
        return RolloutStep::Complete;
    }

    // When partition < replicas, verify the previously released member (ordinal = partition).
    if observed.partition < observed.replicas {
        let active_member = observed.partition;
        let member = observed.members.get(&active_member);

        let runs_update = member
            .map(|m| m.revision == observed.update_revision)
            .unwrap_or(false);
        if !runs_update {
            return RolloutStep::Hold {
                reason: HoldReason::MemberNotUpdated,
            };
        }

        let is_ready = member.map(|m| m.ready).unwrap_or(false);
        if !is_ready {
            return RolloutStep::Hold {
                reason: HoldReason::MemberNotReady,
            };
        }

        let is_healthy = gate
            .app_healthy
            .get(&active_member)
            .copied()
            .unwrap_or(false);
        if !is_healthy {
            return RolloutStep::Hold {
                reason: HoldReason::MemberUnhealthy,
            };
        }

        let next_partition = observed.partition - 1;
        RolloutStep::SetPartition {
            partition: next_partition,
            member: next_partition,
        }
    } else {
        // partition >= replicas: start rollout by releasing the highest ordinal.
        if observed.replicas == 0 {
            RolloutStep::Complete
        } else {
            let next_partition = observed.replicas - 1;
            RolloutStep::SetPartition {
                partition: next_partition,
                member: next_partition,
            }
        }
    }
}
