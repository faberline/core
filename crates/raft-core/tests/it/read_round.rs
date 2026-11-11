//! ReadIndex 1/3 (#108): the leader's read round and its quorum check.
//!
//! The helpers are a minimal copy of the in-process cluster in `consensus.rs`
//! (that one is private to its module).

use std::collections::HashMap;

use raft_core::{auto_membership, NodeId, Outgoing, RaftNode, ReadRefused};

struct Cluster {
    nodes: HashMap<NodeId, RaftNode>,
}

impl Cluster {
    fn new(n: u64) -> Cluster {
        let m = auto_membership(n);
        Cluster {
            nodes: (0..n)
                .map(NodeId::new)
                .map(|id| (id, RaftNode::new(id, &m)))
                .collect(),
        }
    }

    fn node(&mut self, id: NodeId) -> &mut RaftNode {
        self.nodes.get_mut(&id).unwrap()
    }

    fn take(&mut self, id: NodeId) -> Vec<Outgoing> {
        self.node(id).take_outgoing()
    }

    /// Deliver `msgs`, sent by `from`, to their targets. Returns the replies
    /// the other nodes queued, without delivering them.
    fn deliver(&mut self, from: NodeId, msgs: Vec<Outgoing>) -> Vec<(NodeId, Outgoing)> {
        for o in msgs {
            self.node(o.to).handle(from, o.msg);
        }
        let mut replies = Vec::new();
        for (id, node) in self.nodes.iter_mut() {
            if *id != from {
                for o in node.take_outgoing() {
                    replies.push((*id, o));
                }
            }
        }
        replies
    }

    /// Deliver replies that were held back by `deliver`.
    fn deliver_replies(&mut self, replies: Vec<(NodeId, Outgoing)>) {
        for (from, o) in replies {
            self.node(o.to).handle(from, o.msg);
        }
    }

    fn pump(&mut self) {
        let mut guard = 0;
        loop {
            let mut msgs = Vec::new();
            for (id, node) in self.nodes.iter_mut() {
                for o in node.take_outgoing() {
                    msgs.push((*id, o));
                }
            }
            if msgs.is_empty() {
                break;
            }
            for (from, o) in msgs {
                self.node(o.to).handle(from, o.msg);
            }
            guard += 1;
            assert!(guard < 10_000, "pump did not converge");
        }
    }

    fn tick(&mut self) {
        for n in self.nodes.values_mut() {
            n.tick();
        }
        self.pump();
    }

    fn leader(&self) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|(_, n)| n.is_leader())
            .map(|(id, _)| *id)
    }

    fn run_until_leader(&mut self) -> NodeId {
        for _ in 0..200 {
            self.tick();
            if let Some(l) = self.leader() {
                return l;
            }
        }
        panic!("no leader elected");
    }

    fn propose(&mut self, cmd: Vec<u8>) {
        let l = self.leader().expect("a leader");
        self.node(l).propose(cmd).expect("leader accepts");
        self.pump();
        self.tick();
        self.tick();
    }
}

#[test]
fn leader_without_own_term_commit_refuses() {
    let mut c = Cluster::new(3);
    let l = c.run_until_leader();
    assert_eq!(
        c.node(l).start_read_round(),
        Err(ReadRefused::NoCommitInTerm)
    );
    c.propose(vec![1]);
    assert_eq!(c.node(l).start_read_round(), Ok(1));
}

#[test]
fn non_leader_refuses() {
    let mut c = Cluster::new(3);
    let l = c.run_until_leader();
    let follower = (0..3)
        .map(NodeId::new)
        .find(|id| *id != l)
        .expect("a follower");
    assert_eq!(
        c.node(follower).start_read_round(),
        Err(ReadRefused::NotLeader)
    );
}

#[test]
fn quorum_ack_of_current_round() {
    let mut c = Cluster::new(3);
    let l = c.run_until_leader();
    c.propose(vec![1]);
    let r = c.node(l).start_read_round().unwrap();
    assert!(!c.node(l).read_round_acked(r));
    c.pump();
    assert!(c.node(l).read_round_acked(r));
    assert!(!c.node(l).read_round_acked(r + 1));
    assert!(!c.node(l).read_round_acked(0));
}

#[test]
fn reply_from_earlier_round_does_not_count() {
    let mut c = Cluster::new(3);
    let l = c.run_until_leader();
    c.propose(vec![1]);

    // Round 1 reaches the followers; their replies are held back.
    assert_eq!(c.node(l).start_read_round(), Ok(1));
    let round1 = c.take(l);
    let stale = c.deliver(l, round1);

    // Round 2 starts before the round 1 replies arrive.
    assert_eq!(c.node(l).start_read_round(), Ok(2));
    let round2 = c.take(l);
    let fresh = c.deliver(l, round2);

    // The stale round 1 replies arrive: they must not ack any round.
    c.deliver_replies(stale);
    assert!(!c.node(l).read_round_acked(2));
    assert!(!c.node(l).read_round_acked(1));

    // The round 2 replies arrive: round 2 is now acked by a quorum.
    c.deliver_replies(fresh);
    assert!(c.node(l).read_round_acked(2));
}

#[test]
fn single_voter_acks_at_once() {
    let mut n = RaftNode::new(NodeId::new(0), &auto_membership(1));
    for _ in 0..200 {
        n.tick();
        if n.is_leader() {
            break;
        }
    }
    assert!(n.is_leader());
    n.propose(vec![1]).expect("leader accepts");
    assert_eq!(n.take_committed().len(), 1);
    assert_eq!(n.start_read_round(), Ok(1));
    assert!(n.read_round_acked(1));
}
