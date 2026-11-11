use super::*;
use crate::domain::entry::EntryKind;
use crate::domain::membership::auto_membership;
use crate::domain::message::{AppendResp, RaftMsg, VoteResp};
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
    assert_eq!(node.propose(vec![11]), Some(Index::new(1)));
    assert_eq!(node.propose(vec![22]), Some(Index::new(2)));
    node
}

#[test]
fn committed_identity_is_stable_until_exact_completion() {
    let mut node = committed_pair();
    let term = node.current_term();
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((Index::new(1), term, EntryKind::Command))
    );
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((Index::new(1), term, EntryKind::Command))
    );
    assert!(!node.finish_committed_identity(Index::new(2), term));
    assert!(!node.finish_committed_identity(Index::new(1), term.next()));
    assert_eq!(node.last_applied, Index::new(0));
    assert!(node.finish_committed_identity(Index::new(1), term));
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((Index::new(2), term, EntryKind::Command))
    );
    assert!(!node.finish_committed_identity(Index::new(1), term));
    assert!(node.finish_committed_identity(Index::new(2), term));
    assert_eq!(node.peek_next_committed_identity(), None);
    assert!(!node.finish_committed_identity(Index::new(3), term));
}

#[test]
fn unfinished_committed_head_survives_durable_recovery() {
    let node = committed_pair();
    let membership = node.conf_state().membership.clone();
    let restored = RaftNode::from_persisted(NodeId::new(1), &membership, node.persisted());
    assert_eq!(
        restored.peek_next_committed_identity(),
        Some((Index::new(1), node.current_term(), EntryKind::Command))
    );
    assert_eq!(restored.last_applied, Index::new(0));
}

#[test]
fn configuration_changes_only_when_its_exact_identity_finishes() {
    let mut node = committed_pair();
    let term = node.current_term();
    assert!(node.finish_committed_identity(Index::new(1), term));
    assert!(node.finish_committed_identity(Index::new(2), term));
    let mut next = node.conf_state().clone();
    next.generation += 1;
    let (voters, mut learners) = next.membership.into_parts();
    learners.push(NodeId::new(9));
    next.membership = Membership::new(voters, learners);
    node.log.push(RaftEntry {
        index: Index::new(3),
        term,
        kind: EntryKind::Config,
        command: next.encode(),
    });
    node.commit_index = Index::new(3);
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((Index::new(3), term, EntryKind::Config))
    );
    assert!(!node
        .conf_state()
        .membership
        .learners()
        .contains(&NodeId::new(9)));
    assert!(!node.finish_committed_identity(Index::new(3), term.next()));
    assert!(!node
        .conf_state()
        .membership
        .learners()
        .contains(&NodeId::new(9)));
    assert!(node.finish_committed_identity(Index::new(3), term));
    assert!(node
        .conf_state()
        .membership
        .learners()
        .contains(&NodeId::new(9)));
    assert_eq!(node.peek_next_committed_identity(), None);
}

#[test]
fn public_timing_constants_are_nonzero_and_ordered() {
    const {
        assert!(ELECTION_TIMEOUT_FLOOR_TICKS != 0);
        assert!(HEARTBEAT_INTERVAL_TICKS != 0);
        assert!(HEARTBEAT_INTERVAL_TICKS < ELECTION_TIMEOUT_FLOOR_TICKS);
    }
}

#[test]
fn committed_noop_is_not_returned_by_take_committed() {
    let mut node = committed_pair();
    let term = node.current_term();
    node.log.push(RaftEntry {
        index: Index::new(3),
        term,
        kind: EntryKind::Noop,
        command: Vec::new(),
    });
    node.commit_index = Index::new(3);

    let out = node.take_committed();
    let indexes: Vec<Index> = out.iter().map(|e| e.index).collect();
    assert_eq!(indexes, vec![Index::new(1), Index::new(2)]);
    assert!(out.iter().all(|e| e.kind == EntryKind::Command));
    assert_eq!(node.last_applied, Index::new(3));
}

#[test]
fn committed_noop_identity_finishes_without_effect() {
    let mut node = committed_pair();
    let term = node.current_term();
    node.log.push(RaftEntry {
        index: Index::new(3),
        term,
        kind: EntryKind::Noop,
        command: Vec::new(),
    });
    node.commit_index = Index::new(3);

    assert!(node.finish_committed_identity(Index::new(1), term));
    assert!(node.finish_committed_identity(Index::new(2), term));
    assert_eq!(
        node.peek_next_committed_identity(),
        Some((Index::new(3), term, EntryKind::Noop))
    );
    let conf_before = node.conf_state().clone();
    assert!(node.finish_committed_identity(Index::new(3), term));
    assert_eq!(node.last_applied, Index::new(3));
    assert_eq!(node.conf_state(), &conf_before);
    assert_eq!(node.peek_next_committed_identity(), None);
}

/// Node 1 of voters {1,2,3} holding an uncommitted term-1 entry at index 1,
/// elected leader of term 2 with the vote of node 2.
fn elected_with_old_entry(noop: bool) -> RaftNode {
    let membership = Membership::new(
        vec![NodeId::new(1), NodeId::new(2), NodeId::new(3)],
        Vec::new(),
    );
    let mut node = RaftNode::new(NodeId::new(1), &membership);
    node.set_leader_noop(noop);
    node.current_term = Term::new(1);
    node.log.push(RaftEntry {
        term: Term::new(1),
        index: Index::new(1),
        command: vec![7],
        kind: EntryKind::Command,
    });
    node.start_election();
    node.handle(
        NodeId::new(2),
        RaftMsg::VoteResp(VoteResp {
            term: Term::new(2),
            granted: true,
        }),
    );
    assert_eq!(node.role(), Role::Leader);
    node
}

#[test]
fn leader_noop_is_off_by_default() {
    let mut node = RaftNode::new(NodeId::new(1), &auto_membership(3));
    assert!(!node.leader_noop());
    node.set_leader_noop(true);
    assert!(node.leader_noop());
}

#[test]
fn leader_noop_appends_one_noop_for_the_new_term() {
    let node = elected_with_old_entry(true);
    assert_eq!(node.last_index(), Index::new(2));
    assert_eq!(node.log.len(), 2);
    assert_eq!(node.log[1].kind, EntryKind::Noop);
    assert_eq!(node.log[1].term, Term::new(2));
    assert_eq!(node.log[1].index, Index::new(2));
    assert!(node.log[1].command.is_empty());
}

#[test]
fn leader_noop_sends_the_noop_to_both_peers() {
    let mut node = elected_with_old_entry(true);
    let appends: Vec<_> = node
        .take_outgoing()
        .into_iter()
        .filter_map(|out| match out.msg {
            RaftMsg::Append(req) => Some((out.to, req)),
            _ => None,
        })
        .collect();
    assert_eq!(appends.len(), 2, "one append to each peer");
    for (to, req) in &appends {
        assert!(
            req.entries
                .iter()
                .any(|e| e.kind == EntryKind::Noop && e.index == Index::new(2)),
            "append to {to:?} carries the no-op at index 2"
        );
    }
}

#[test]
fn leader_noop_commits_previous_term_entries_without_a_proposal() {
    let mut node = elected_with_old_entry(true);
    assert_eq!(node.commit_index(), Index::new(0));

    node.handle(
        NodeId::new(2),
        RaftMsg::AppendResp(AppendResp {
            term: Term::new(2),
            success: true,
            match_index: Index::new(2),
            read_round: 0,
        }),
    );
    assert_eq!(node.commit_index(), Index::new(2));

    let applied = node.take_committed();
    assert_eq!(applied.len(), 1, "the no-op is withheld from the consumer");
    assert_eq!(applied[0].index, Index::new(1));
    assert_eq!(applied[0].command, vec![7]);
    assert_eq!(applied[0].kind, EntryKind::Command);
    assert_eq!(node.last_applied, Index::new(2));
}

#[test]
fn without_leader_noop_previous_term_entries_stay_uncommitted() {
    let mut node = elected_with_old_entry(false);
    assert_eq!(node.last_index(), Index::new(1));

    node.handle(
        NodeId::new(2),
        RaftMsg::AppendResp(AppendResp {
            term: Term::new(2),
            success: true,
            match_index: Index::new(1),
            read_round: 0,
        }),
    );
    assert_eq!(
        node.commit_index(),
        Index::new(0),
        "an earlier-term entry cannot commit by counting replicas"
    );
}

#[test]
fn sole_voter_commits_its_leader_noop_at_once() {
    let membership = Membership::new(vec![NodeId::new(1)], Vec::new());
    let mut node = RaftNode::new(NodeId::new(1), &membership);
    node.set_leader_noop(true);
    node.become_leader();

    assert_eq!(node.last_index(), Index::new(1));
    assert_eq!(node.commit_index(), Index::new(1));
    assert!(node.take_committed().is_empty());
    assert_eq!(node.propose(vec![5]), Some(Index::new(2)));
}
