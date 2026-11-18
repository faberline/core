//! Election-visible state changes of one node, derived from its view before
//! and after a step. Logging is info-level and only for these rare changes.

use raft_core::{NodeId, RaftNode, Role, Term};

/// The part of a node that an election log line reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NodeView {
    pub(crate) role: Role,
    pub(crate) term: Term,
    pub(crate) leader: Option<NodeId>,
}

impl NodeView {
    pub(crate) fn of(node: &RaftNode) -> Self {
        Self {
            role: node.role(),
            term: node.current_term(),
            leader: node.leader(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transition {
    ElectionStarted {
        old_term: Term,
        new_term: Term,
    },
    TermChanged {
        old_term: Term,
        new_term: Term,
    },
    LeaderChanged {
        old_leader: Option<NodeId>,
        new_leader: Option<NodeId>,
        term: Term,
    },
}

/// The transitions between two views of one node. Equal views give none.
pub(crate) fn transitions(before: NodeView, after: NodeView) -> Vec<Transition> {
    let mut out = Vec::new();
    if after.term > before.term && after.role == Role::Candidate {
        out.push(Transition::ElectionStarted {
            old_term: before.term,
            new_term: after.term,
        });
    } else if after.term != before.term {
        out.push(Transition::TermChanged {
            old_term: before.term,
            new_term: after.term,
        });
    }
    if after.leader != before.leader {
        out.push(Transition::LeaderChanged {
            old_leader: before.leader,
            new_leader: after.leader,
            term: after.term,
        });
    }
    out
}

/// Writes one info line per transition, under the `raft_runtime::election`
/// target, with structured fields.
pub(crate) fn log_transitions(id: NodeId, group: &str, list: &[Transition]) {
    for transition in list {
        match *transition {
            Transition::ElectionStarted { old_term, new_term } => tracing::info!(
                target: "raft_runtime::election",
                node = id.get(),
                group,
                old_term = old_term.get(),
                new_term = new_term.get(),
                reason = "election timeout or timeout-now",
                "raft: election started"
            ),
            Transition::TermChanged { old_term, new_term } => tracing::info!(
                target: "raft_runtime::election",
                node = id.get(),
                group,
                old_term = old_term.get(),
                new_term = new_term.get(),
                "raft: term changed"
            ),
            Transition::LeaderChanged {
                old_leader,
                new_leader,
                term,
            } => {
                let old = old_leader.map(NodeId::get);
                let new = new_leader.map(NodeId::get);
                tracing::info!(
                    target: "raft_runtime::election",
                    node = id.get(),
                    group,
                    term = term.get(),
                    old_leader = ?old,
                    new_leader = ?new,
                    "raft: leader changed"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(role: Role, term: u64, leader: Option<u64>) -> NodeView {
        NodeView {
            role,
            term: Term::new(term),
            leader: leader.map(NodeId::new),
        }
    }

    #[test]
    fn equal_views_give_no_transition() {
        let v = view(Role::Follower, 3, Some(1));
        assert!(transitions(v, v).is_empty());
    }

    #[test]
    fn new_candidate_term_is_an_election_start() {
        let out = transitions(
            view(Role::Follower, 1, Some(1)),
            view(Role::Candidate, 2, None),
        );
        assert_eq!(
            out,
            vec![
                Transition::ElectionStarted {
                    old_term: Term::new(1),
                    new_term: Term::new(2),
                },
                Transition::LeaderChanged {
                    old_leader: Some(NodeId::new(1)),
                    new_leader: None,
                    term: Term::new(2),
                },
            ]
        );
    }

    #[test]
    fn higher_term_without_candidacy_is_a_term_change() {
        let out = transitions(
            view(Role::Leader, 2, Some(0)),
            view(Role::Follower, 3, None),
        );
        assert_eq!(
            out,
            vec![
                Transition::TermChanged {
                    old_term: Term::new(2),
                    new_term: Term::new(3),
                },
                Transition::LeaderChanged {
                    old_leader: Some(NodeId::new(0)),
                    new_leader: None,
                    term: Term::new(3),
                },
            ]
        );
    }

    #[test]
    fn same_term_new_leader_is_only_a_leader_change() {
        let out = transitions(
            view(Role::Follower, 4, None),
            view(Role::Follower, 4, Some(2)),
        );
        assert_eq!(
            out,
            vec![Transition::LeaderChanged {
                old_leader: None,
                new_leader: Some(NodeId::new(2)),
                term: Term::new(4),
            }]
        );
    }

    #[test]
    fn winning_the_election_changes_only_the_leader() {
        let out = transitions(
            view(Role::Candidate, 2, None),
            view(Role::Leader, 2, Some(0)),
        );
        assert_eq!(
            out,
            vec![Transition::LeaderChanged {
                old_leader: None,
                new_leader: Some(NodeId::new(0)),
                term: Term::new(2),
            }]
        );
    }
}
