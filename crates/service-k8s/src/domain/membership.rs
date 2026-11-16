//! Pure decision core for one membership transition step of a replicated shard.
//!
//! [`plan_membership_step`] looks at the observed Raft membership of every shard
//! and at the desired replica layout, and returns exactly one next step. It owns
//! the order of the work: scale up (`ScalePods`, `AddLearner`, `Promote`), scale
//! down (`TransferLeadership`, `Demote`, `RemoveMember`, `ScalePods`,
//! `DeletePvc`), one layer at a time across shards, and recovery from an
//! interrupted transition. It does no I/O.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

/// Shard index within the service.
pub type Shard = u32;
/// Replica index within a shard.
pub type Replica = u32;

/// Role of a committed Raft member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberRole {
    Voter,
    Learner,
}

/// Observed state of one committed Raft member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberState {
    pub role: MemberRole,
    pub caught_up: bool,
}

/// Observed state of one shard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShardMembership {
    /// Committed Raft members by replica index.
    pub members: BTreeMap<Replica, MemberState>,
    pub leader: Option<Replica>,
    /// Pods `0..pods` exist.
    pub pods: u32,
    /// Replica indexes that have a PVC.
    pub pvcs: BTreeSet<Replica>,
}

/// Observed membership of every shard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedMembership {
    pub shards: BTreeMap<Shard, ShardMembership>,
}

/// How reads scale beyond the voters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadScalingMode {
    /// Every replica is a voter.
    None,
    /// `voters` stays fixed; scaling adds or removes learners.
    Learners,
}

/// Desired replica layout of every shard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesiredMembership {
    pub replicas_per_shard: u32,
    pub voters: u32,
    pub read_scaling: ReadScalingMode,
}

/// The single next step of a membership transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MembershipStep {
    AddLearner {
        shard: Shard,
        replica: Replica,
    },
    Promote {
        shard: Shard,
        replica: Replica,
    },
    TransferLeadership {
        shard: Shard,
        from: Replica,
        to: Replica,
    },
    Demote {
        shard: Shard,
        replica: Replica,
    },
    RemoveMember {
        shard: Shard,
        replica: Replica,
    },
    ScalePods {
        shard: Shard,
        replicas: u32,
    },
    DeletePvc {
        shard: Shard,
        replica: Replica,
    },
    /// No step now: finished, or waiting (for example a learner that is not caught up yet).
    Done,
}

impl ShardMembership {
    /// The largest of the pod count, the member indexes and the PVC indexes, as a count.
    fn size(&self) -> u32 {
        let members = self.members.keys().next_back().map_or(0, |&i| i + 1);
        let pvcs = self.pvcs.iter().next_back().map_or(0, |&i| i + 1);
        self.pods.max(members).max(pvcs)
    }

    fn voters(&self) -> u32 {
        let count = self
            .members
            .values()
            .filter(|m| m.role == MemberRole::Voter)
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// The number of voters the shard must have at layer `n`.
    fn voter_target(&self, n: u32, desired: &DesiredMembership) -> u32 {
        match desired.read_scaling {
            ReadScalingMode::None => n,
            ReadScalingMode::Learners => desired.voters.min(n),
        }
    }

    /// True when the shard is complete at layer `n`.
    fn complete(&self, n: u32, desired: &DesiredMembership) -> bool {
        self.pods >= n
            && (0..n).all(|i| self.members.contains_key(&i))
            && self.voters() >= self.voter_target(n, desired)
    }

    /// The largest layer `0..=replicas_per_shard` at which the shard is complete.
    fn level(&self, desired: &DesiredMembership) -> u32 {
        (0..=desired.replicas_per_shard)
            .rev()
            .find(|&n| self.complete(n, desired))
            .unwrap_or(0)
    }

    /// The lowest-index voter other than `except`.
    fn lowest_voter_except(&self, except: Replica) -> Option<Replica> {
        self.members
            .iter()
            .find(|(&i, m)| i != except && m.role == MemberRole::Voter)
            .map(|(&i, _)| i)
    }
}

/// Plans the next membership step from the observed state and the desired layout.
pub fn plan_membership_step(
    observed: &ObservedMembership,
    desired: &DesiredMembership,
) -> MembershipStep {
    let r = desired.replicas_per_shard;
    if r == 0 || observed.shards.is_empty() {
        return MembershipStep::Done;
    }

    // Scale down first: a shard with more replicas than the desired count.
    let over_r = observed.shards.values().any(|s| s.size() > r);
    if over_r {
        let t = observed
            .shards
            .values()
            .map(|s| s.size().saturating_sub(1))
            .max()
            .unwrap_or(0)
            .max(r);
        for (&shard, s) in &observed.shards {
            if s.size() <= t {
                continue;
            }
            if let Some(step) = scale_down_step(shard, s) {
                return step;
            }
        }
        return MembershipStep::Done;
    }

    // Scale up one layer at a time across shards.
    let min_level = observed
        .shards
        .values()
        .map(|s| s.level(desired))
        .min()
        .unwrap_or(0);
    if min_level < r {
        let t = r.min(min_level + 1);
        for (&shard, s) in &observed.shards {
            if s.complete(t, desired) {
                continue;
            }
            if let Some(step) = scale_up_step(shard, s, t, desired) {
                return step;
            }
        }
    }
    MembershipStep::Done
}

/// The step that takes the highest replica of `s` out, or `None` when nothing applies.
fn scale_down_step(shard: Shard, s: &ShardMembership) -> Option<MembershipStep> {
    let top = s.size() - 1;
    if let Some(member) = s.members.get(&top) {
        if s.leader == Some(top) {
            return s
                .lowest_voter_except(top)
                .map(|to| MembershipStep::TransferLeadership {
                    shard,
                    from: top,
                    to,
                });
        }
        return Some(match member.role {
            MemberRole::Voter => MembershipStep::Demote {
                shard,
                replica: top,
            },
            MemberRole::Learner => MembershipStep::RemoveMember {
                shard,
                replica: top,
            },
        });
    }
    if s.pods > top {
        return Some(MembershipStep::ScalePods {
            shard,
            replicas: top,
        });
    }
    if s.pvcs.contains(&top) {
        return Some(MembershipStep::DeletePvc {
            shard,
            replica: top,
        });
    }
    None
}

/// The step that brings the shard to layer `t`, or `None` when it must wait.
fn scale_up_step(
    shard: Shard,
    s: &ShardMembership,
    t: u32,
    desired: &DesiredMembership,
) -> Option<MembershipStep> {
    if s.pods < t {
        return Some(MembershipStep::ScalePods { shard, replicas: t });
    }
    if let Some(replica) = (0..t).find(|i| !s.members.contains_key(i)) {
        return Some(MembershipStep::AddLearner { shard, replica });
    }
    if s.voters() < s.voter_target(t, desired) {
        let learner = s
            .members
            .iter()
            .find(|(_, m)| m.role == MemberRole::Learner && m.caught_up)
            .map(|(&i, _)| i)?;
        return Some(MembershipStep::Promote {
            shard,
            replica: learner,
        });
    }
    None
}
