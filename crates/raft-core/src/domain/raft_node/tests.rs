use super::*;
use crate::domain::entry::EntryKind;
use crate::domain::membership::auto_membership;
use crate::domain::timing::HEARTBEAT_INTERVAL_TICKS;

#[test]
fn new_derives_election_timeout_from_public_floor_and_node_id() {
    let membership = auto_membership(3);
    let id = NodeId::new(7);

    let node = RaftNode::new(id, &membership);

    assert_eq!(
        node.election_timeout,
        ELECTION_TIMEOUT_FLOOR_TICKS + id.get()
    );
}

#[test]
fn leader_sends_first_periodic_heartbeat_at_public_interval() {
    let membership = Membership::new(vec![NodeId::new(1), NodeId::new(2)], Vec::new());
    let mut node = RaftNode::new(NodeId::new(1), &membership);
    node.become_leader();
    assert_eq!(
        node.take_outgoing().len(),
        1,
        "leader sends its initial append"
    );

    for _ in 0..HEARTBEAT_INTERVAL_TICKS - 1 {
        node.tick();
        assert!(
            node.take_outgoing().is_empty(),
            "heartbeat must not be early"
        );
    }

    node.tick();
    assert_eq!(
        node.take_outgoing().len(),
        1,
        "heartbeat is due at the interval"
    );
}

fn committed_pair() -> RaftNode {
    let membership = Membership::new(vec![NodeId::new(1)], vec![]);
    let mut node = RaftNode::new(NodeId::new(1), &membership);
    node.become_leader();
    assert_eq!(node.propose(vec![11]), Some(1));
    assert_eq!(node.propose(vec![22]), Some(2));
    node
}

#[test]
fn committed_identity_is_stable_until_exact_completion() {
    let mut node = committed_pair();
    let term = node.current_term();
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((1, term, EntryKind::Command))
    );
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((1, term, EntryKind::Command))
    );
    assert!(!node.finish_committed_identity(2, term));
    assert!(!node.finish_committed_identity(1, term + 1));
    assert_eq!(node.last_applied, 0);
    assert!(node.finish_committed_identity(1, term));
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((2, term, EntryKind::Command))
    );
    assert!(!node.finish_committed_identity(1, term));
    assert!(node.finish_committed_identity(2, term));
    assert_eq!(node.peek_next_committed_identity(), None);
    assert!(!node.finish_committed_identity(3, term));
}

#[test]
fn unfinished_committed_head_survives_durable_recovery() {
    let node = committed_pair();
    let membership = node.conf_state().membership.clone();
    let restored = RaftNode::from_persisted(NodeId::new(1), &membership, node.persisted());
    assert_eq!(
        restored.peek_next_committed_identity(),
        Some((1, node.current_term(), EntryKind::Command))
    );
    assert_eq!(restored.last_applied, 0);
}

#[test]
fn configuration_changes_only_when_its_exact_identity_finishes() {
    let mut node = committed_pair();
    let term = node.current_term();
    assert!(node.finish_committed_identity(1, term));
    assert!(node.finish_committed_identity(2, term));
    let mut next = node.conf_state().clone();
    next.generation += 1;
    let (voters, mut learners) = next.membership.into_parts();
    learners.push(NodeId::new(9));
    next.membership = Membership::new(voters, learners);
    node.log.push(RaftEntry {
        index: 3,
        term,
        kind: EntryKind::Config,
        command: next.encode(),
    });
    node.commit_index = 3;
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((3, term, EntryKind::Config))
    );
    assert!(!node
        .conf_state()
        .membership
        .learners()
        .contains(&NodeId::new(9)));
    assert!(!node.finish_committed_identity(3, term + 1));
    assert!(!node
        .conf_state()
        .membership
        .learners()
        .contains(&NodeId::new(9)));
    assert!(node.finish_committed_identity(3, term));
    assert!(node
        .conf_state()
        .membership
        .learners()
        .contains(&NodeId::new(9)));
    assert_eq!(node.peek_next_committed_identity(), None);
}

#[test]
fn public_timing_constants_are_nonzero_and_ordered() {
    assert_ne!(ELECTION_TIMEOUT_FLOOR_TICKS, 0);
    assert_ne!(HEARTBEAT_INTERVAL_TICKS, 0);
    assert!(HEARTBEAT_INTERVAL_TICKS < ELECTION_TIMEOUT_FLOOR_TICKS);
}
