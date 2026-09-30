use super::*;
use crate::Term;

fn fresh_node(membership: Membership) -> RaftNode {
    RaftNode::new(NodeId::new(0), &membership)
}

#[test]
fn tick_persists_initial_learner_image_then_skips_unchanged_tick() {
    let mut node = fresh_node(Membership::new(vec![], vec![NodeId::new(0)]));
    let initial = node.persisted();
    let mut first_tick = true;
    let mut calls = 0;

    assert!(tick_then_maybe_persist(
        &mut node,
        &mut first_tick,
        |persisted| {
            calls += 1;
            assert_eq!(persisted.persisted(), initial);
            true
        },
        || false,
    ));
    assert_eq!(calls, 1, "first learner tick persists fresh membership");
    assert_eq!(node.persisted(), initial, "timers are not durable state");

    assert!(tick_then_maybe_persist(
        &mut node,
        &mut first_tick,
        |_| panic!("unchanged learner tick must not persist"),
        || false,
    ));
    assert_eq!(node.persisted(), initial, "unchanged tick keeps full image");
}

#[test]
fn tick_persists_election_image_and_failure_blocks_apply_and_stays_latched() {
    let mut node = fresh_node(Membership::new(vec![NodeId::new(0)], vec![]));
    let mut first_tick = true;
    let mut calls = 0;
    for _ in 0..raft_core::ELECTION_TIMEOUT_FLOOR_TICKS - 1 {
        assert!(tick_then_maybe_persist(
            &mut node,
            &mut first_tick,
            |_| {
                calls += 1;
                true
            },
            || false,
        ));
    }
    assert_eq!(calls, 1, "only startup has persisted before election");

    let latched = std::cell::Cell::new(false);
    assert!(!tick_then_maybe_persist(
        &mut node,
        &mut first_tick,
        |persisted| {
            calls += 1;
            let image = persisted.persisted();
            assert_eq!(image.term, Term::new(1));
            assert_eq!(image.voted_for, Some(NodeId::new(0)));
            latched.set(true);
            false
        },
        || latched.get(),
    ));
    assert_eq!(calls, 2, "term-changing election persists");
    assert!(!tick_then_maybe_persist(
        &mut node,
        &mut first_tick,
        |_| panic!("latched unchanged tick must not persist"),
        || true,
    ));
}

#[test]
fn tick_persists_joint_election_with_leave_joint_entry() {
    let membership = Membership::new(vec![NodeId::new(0)], vec![]);
    let mut node = fresh_node(membership.clone());
    assert!(node.adopt_conf(raft_core::ConfState {
        membership,
        outgoing: Some(vec![NodeId::new(0)]),
        generation: 1,
    }));
    let mut first_tick = true;
    for _ in 0..raft_core::ELECTION_TIMEOUT_FLOOR_TICKS - 1 {
        assert!(tick_then_maybe_persist(
            &mut node,
            &mut first_tick,
            |_| true,
            || false,
        ));
    }
    assert!(tick_then_maybe_persist(
        &mut node,
        &mut first_tick,
        |persisted| {
            let image = persisted.persisted();
            assert_eq!(image.term, Term::new(1));
            assert!(image
                .conf
                .as_ref()
                .is_some_and(|conf| conf.outgoing.is_some()));
            assert!(matches!(
                image.log.last().map(|entry| entry.kind),
                Some(raft_core::EntryKind::Config)
            ));
            true
        },
        || false,
    ));
}

#[test]
fn elected_leader_heartbeat_ticks_skip_persistence_and_keep_full_image() {
    let mut node = fresh_node(Membership::new(vec![NodeId::new(0)], vec![]));
    let mut first_tick = true;
    for _ in 0..raft_core::ELECTION_TIMEOUT_FLOOR_TICKS - 1 {
        assert!(tick_then_maybe_persist(
            &mut node,
            &mut first_tick,
            |_| true,
            || false,
        ));
    }
    assert!(tick_then_maybe_persist(
        &mut node,
        &mut first_tick,
        |_| true,
        || false,
    ));
    assert!(node.is_leader());
    let leader_image = node.persisted();

    for _ in 0..raft_core::HEARTBEAT_INTERVAL_TICKS * 2 {
        assert!(tick_then_maybe_persist(
            &mut node,
            &mut first_tick,
            |_| panic!("idle leader heartbeat tick must not persist"),
            || false,
        ));
        assert_eq!(
            node.persisted(),
            leader_image,
            "heartbeat timers and outbox must not alter the durable image"
        );
    }
}
