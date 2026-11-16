use std::collections::{BTreeMap, BTreeSet};

use super::*;

const V: MemberRole = MemberRole::Voter;
const L: MemberRole = MemberRole::Learner;

fn m(role: MemberRole, caught_up: bool) -> MemberState {
    MemberState { role, caught_up }
}

fn shard(
    members: &[(u32, MemberRole, bool)],
    leader: Option<u32>,
    pods: u32,
    pvcs: &[u32],
) -> ShardMembership {
    ShardMembership {
        members: members
            .iter()
            .map(|&(replica, role, caught_up)| (replica, m(role, caught_up)))
            .collect::<BTreeMap<_, _>>(),
        leader,
        pods,
        pvcs: pvcs.iter().copied().collect::<BTreeSet<_>>(),
    }
}

fn one(s: ShardMembership) -> ObservedMembership {
    ObservedMembership {
        shards: BTreeMap::from([(0, s)]),
    }
}

fn want(r: u32, voters: u32, mode: ReadScalingMode) -> DesiredMembership {
    DesiredMembership {
        replicas_per_shard: r,
        voters,
        read_scaling: mode,
    }
}

#[test]
fn scale_up_one_to_three_walks_scale_add_promote() {
    let d = want(3, 3, ReadScalingMode::None);
    let rows = [
        (
            shard(&[(0, V, true)], Some(0), 1, &[0]),
            MembershipStep::ScalePods {
                shard: 0,
                replicas: 2,
            },
        ),
        (
            shard(&[(0, V, true)], Some(0), 2, &[0, 1]),
            MembershipStep::AddLearner {
                shard: 0,
                replica: 1,
            },
        ),
        (
            shard(&[(0, V, true), (1, L, false)], Some(0), 2, &[0, 1]),
            MembershipStep::Done,
        ),
        (
            shard(&[(0, V, true), (1, L, true)], Some(0), 2, &[0, 1]),
            MembershipStep::Promote {
                shard: 0,
                replica: 1,
            },
        ),
        (
            shard(&[(0, V, true), (1, V, true)], Some(0), 2, &[0, 1]),
            MembershipStep::ScalePods {
                shard: 0,
                replicas: 3,
            },
        ),
        (
            shard(&[(0, V, true), (1, V, true)], Some(0), 3, &[0, 1, 2]),
            MembershipStep::AddLearner {
                shard: 0,
                replica: 2,
            },
        ),
        (
            shard(
                &[(0, V, true), (1, V, true), (2, L, true)],
                Some(0),
                3,
                &[0, 1, 2],
            ),
            MembershipStep::Promote {
                shard: 0,
                replica: 2,
            },
        ),
        (
            shard(
                &[(0, V, true), (1, V, true), (2, V, true)],
                Some(0),
                3,
                &[0, 1, 2],
            ),
            MembershipStep::Done,
        ),
    ];
    for (state, expected) in rows {
        assert_eq!(plan_membership_step(&one(state), &d), expected);
    }
}

#[test]
fn scale_down_transfers_leadership_before_demote_and_remove() {
    let d = want(2, 2, ReadScalingMode::None);
    let all_voters = [(0, V, true), (1, V, true), (2, V, true)];

    assert_eq!(
        plan_membership_step(&one(shard(&all_voters, Some(2), 3, &[0, 1, 2])), &d),
        MembershipStep::TransferLeadership {
            shard: 0,
            from: 2,
            to: 0
        }
    );
    assert_eq!(
        plan_membership_step(&one(shard(&all_voters, Some(0), 3, &[0, 1, 2])), &d),
        MembershipStep::Demote {
            shard: 0,
            replica: 2
        }
    );
    assert_eq!(
        plan_membership_step(
            &one(shard(
                &[(0, V, true), (1, V, true), (2, L, true)],
                Some(0),
                3,
                &[0, 1, 2]
            )),
            &d
        ),
        MembershipStep::RemoveMember {
            shard: 0,
            replica: 2
        }
    );
    assert_eq!(
        plan_membership_step(
            &one(shard(&[(0, V, true), (1, V, true)], Some(0), 3, &[0, 1, 2])),
            &d
        ),
        MembershipStep::ScalePods {
            shard: 0,
            replicas: 2
        }
    );
    assert_eq!(
        plan_membership_step(
            &one(shard(&[(0, V, true), (1, V, true)], Some(0), 2, &[0, 1, 2])),
            &d
        ),
        MembershipStep::DeletePvc {
            shard: 0,
            replica: 2
        }
    );
    assert_eq!(
        plan_membership_step(
            &one(shard(&[(0, V, true), (1, V, true)], Some(0), 2, &[0, 1])),
            &d
        ),
        MembershipStep::Done
    );
}

#[test]
fn scale_pods_down_is_never_returned_while_the_member_is_listed() {
    // Negative control: ScalePods down must wait until RemoveMember has committed.
    let d = want(2, 2, ReadScalingMode::None);
    let state = one(shard(
        &[(0, V, true), (1, V, true), (2, L, true)],
        Some(0),
        3,
        &[0, 1, 2],
    ));
    let step = plan_membership_step(&state, &d);
    assert_eq!(
        step,
        MembershipStep::RemoveMember {
            shard: 0,
            replica: 2
        }
    );
    assert!(!matches!(step, MembershipStep::ScalePods { .. }));
}

#[test]
fn delete_pvc_waits_until_the_pod_is_gone() {
    let d = want(2, 2, ReadScalingMode::None);
    assert_eq!(
        plan_membership_step(
            &one(shard(&[(0, V, true), (1, V, true)], Some(0), 3, &[0, 1, 2])),
            &d
        ),
        MembershipStep::ScalePods {
            shard: 0,
            replicas: 2
        }
    );
    assert_eq!(
        plan_membership_step(
            &one(shard(&[(0, V, true), (1, V, true)], Some(0), 2, &[0, 1, 2])),
            &d
        ),
        MembershipStep::DeletePvc {
            shard: 0,
            replica: 2
        }
    );
}

#[test]
fn read_scaling_learners_never_promotes_beyond_voters() {
    let five = want(5, 3, ReadScalingMode::Learners);
    let full = one(shard(
        &[
            (0, V, true),
            (1, V, true),
            (2, V, true),
            (3, L, true),
            (4, L, true),
        ],
        Some(0),
        5,
        &[0, 1, 2, 3, 4],
    ));
    assert_eq!(plan_membership_step(&full, &five), MembershipStep::Done);

    let three = want(3, 3, ReadScalingMode::Learners);
    let short = one(shard(
        &[(0, V, true), (1, V, true), (2, L, true)],
        Some(0),
        3,
        &[0, 1, 2],
    ));
    assert_eq!(
        plan_membership_step(&short, &three),
        MembershipStep::Promote {
            shard: 0,
            replica: 2
        }
    );
}

#[test]
fn a_member_added_but_not_promoted_is_promoted_on_recovery() {
    let d = want(2, 2, ReadScalingMode::None);
    let state = one(shard(&[(0, V, true), (1, L, true)], Some(0), 2, &[0, 1]));
    assert_eq!(
        plan_membership_step(&state, &d),
        MembershipStep::Promote {
            shard: 0,
            replica: 1
        }
    );
}

#[test]
fn a_member_removed_with_its_pod_still_up_continues_with_scale_pods() {
    let d = want(2, 2, ReadScalingMode::None);
    let state = one(shard(&[(0, V, true), (1, V, true)], Some(0), 3, &[0, 1, 2]));
    assert_eq!(
        plan_membership_step(&state, &d),
        MembershipStep::ScalePods {
            shard: 0,
            replicas: 2
        }
    );
}

#[test]
fn layers_never_differ_by_more_than_one() {
    let d = want(3, 3, ReadScalingMode::None);
    let observed = ObservedMembership {
        shards: BTreeMap::from([
            (0, shard(&[(0, V, true), (1, V, true)], Some(0), 2, &[0, 1])),
            (1, shard(&[(0, V, true)], Some(0), 1, &[0])),
        ]),
    };
    assert_eq!(
        plan_membership_step(&observed, &d),
        MembershipStep::ScalePods {
            shard: 1,
            replicas: 2
        }
    );
}

#[test]
fn scale_down_goes_shard_by_shard_in_index_order() {
    let d = want(2, 2, ReadScalingMode::None);
    let full = shard(
        &[(0, V, true), (1, V, true), (2, V, true)],
        Some(0),
        3,
        &[0, 1, 2],
    );
    let observed = ObservedMembership {
        shards: BTreeMap::from([(0, full.clone()), (1, full.clone())]),
    };
    assert_eq!(
        plan_membership_step(&observed, &d),
        MembershipStep::Demote {
            shard: 0,
            replica: 2
        }
    );

    let shrunk = shard(&[(0, V, true), (1, V, true)], Some(0), 2, &[0, 1]);
    let observed = ObservedMembership {
        shards: BTreeMap::from([(0, shrunk), (1, full)]),
    };
    assert_eq!(
        plan_membership_step(&observed, &d),
        MembershipStep::Demote {
            shard: 1,
            replica: 2
        }
    );
}

#[test]
fn nothing_to_plan_is_done() {
    let full = shard(&[(0, V, true), (1, V, true)], Some(0), 2, &[0, 1]);
    assert_eq!(
        plan_membership_step(&one(full), &want(0, 0, ReadScalingMode::None)),
        MembershipStep::Done
    );
    assert_eq!(
        plan_membership_step(
            &ObservedMembership {
                shards: BTreeMap::new()
            },
            &want(2, 2, ReadScalingMode::None)
        ),
        MembershipStep::Done
    );
}
