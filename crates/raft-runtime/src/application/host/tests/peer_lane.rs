use super::*;
use raft_core::{AppendReq, AppendResp, TimeoutNowReq, VoteReq, VoteResp};

fn append(marker: u64) -> RaftMsg {
    RaftMsg::Append(AppendReq {
        term: marker,
        leader: NodeId::new(7),
        prev_log_index: 0,
        prev_log_term: 0,
        entries: vec![],
        leader_commit: marker,
    })
}

fn timeout_now(marker: u64) -> RaftMsg {
    RaftMsg::TimeoutNow(TimeoutNowReq {
        term: marker,
        leader: NodeId::new(7),
    })
}

fn assert_append(message: Option<RaftMsg>, marker: u64) {
    match message {
        Some(RaftMsg::Append(req)) => assert_eq!(req.leader_commit, marker),
        other => panic!("expected Append({marker}), got {other:?}"),
    }
}

fn assert_timeout_now(message: Option<RaftMsg>, term: u64, leader: u64) {
    match message {
        Some(RaftMsg::TimeoutNow(req)) => {
            assert_eq!(req.term, term);
            assert_eq!(req.leader, NodeId::new(leader));
        }
        other => panic!("expected TimeoutNow({term}, {leader}), got {other:?}"),
    }
}

fn assert_same_message(actual: RaftMsg, expected: RaftMsg) {
    match (actual, expected) {
        (RaftMsg::Vote(actual), RaftMsg::Vote(expected)) => {
            assert_eq!(actual.term, expected.term);
            assert_eq!(actual.candidate, expected.candidate);
            assert_eq!(actual.last_log_index, expected.last_log_index);
            assert_eq!(actual.last_log_term, expected.last_log_term);
        }
        (RaftMsg::VoteResp(actual), RaftMsg::VoteResp(expected)) => {
            assert_eq!(actual.term, expected.term);
            assert_eq!(actual.granted, expected.granted);
        }
        (RaftMsg::Append(actual), RaftMsg::Append(expected)) => {
            assert_eq!(actual.term, expected.term);
            assert_eq!(actual.leader, expected.leader);
            assert_eq!(actual.prev_log_index, expected.prev_log_index);
            assert_eq!(actual.prev_log_term, expected.prev_log_term);
            assert_eq!(actual.entries, expected.entries);
            assert_eq!(actual.leader_commit, expected.leader_commit);
        }
        (RaftMsg::AppendResp(actual), RaftMsg::AppendResp(expected)) => {
            assert_eq!(actual.term, expected.term);
            assert_eq!(actual.success, expected.success);
            assert_eq!(actual.match_index, expected.match_index);
        }
        (RaftMsg::InstallSnapshot(actual), RaftMsg::InstallSnapshot(expected)) => {
            assert_eq!(actual.term, expected.term);
            assert_eq!(actual.leader, expected.leader);
            assert_eq!(actual.snapshot_index, expected.snapshot_index);
            assert_eq!(actual.snapshot_term, expected.snapshot_term);
            assert_eq!(actual.data, expected.data);
        }
        (RaftMsg::InstallSnapshotResp(actual), RaftMsg::InstallSnapshotResp(expected)) => {
            assert_eq!(actual.term, expected.term);
            assert_eq!(actual.accepted, expected.accepted);
            assert_eq!(actual.snapshot_index, expected.snapshot_index);
        }
        (RaftMsg::TimeoutNow(actual), RaftMsg::TimeoutNow(expected)) => {
            assert_eq!(actual.term, expected.term);
            assert_eq!(actual.leader, expected.leader);
        }
        (actual, expected) => panic!("expected {expected:?}, got {actual:?}"),
    }
}

#[test]
fn peer_lane_keeps_timeout_now_before_later_append_when_worker_is_occupied() {
    let mut queue = PeerLaneQueue::default();

    queue.enqueue(append(10));
    assert_append(queue.dequeue(), 10); // The lane worker is now in flight.

    queue.enqueue(timeout_now(20));
    queue.enqueue(append(30)); // A later leader heartbeat.

    assert_timeout_now(queue.dequeue(), 20, 7);
    assert_append(queue.dequeue(), 30);
    assert!(queue.dequeue().is_none());
}

#[test]
fn peer_lane_coalesces_only_adjacent_pending_appends() {
    let mut adjacent = PeerLaneQueue::default();
    adjacent.enqueue(append(10));
    adjacent.enqueue(append(20));
    assert_append(adjacent.dequeue(), 20);
    assert!(adjacent.dequeue().is_none());

    let mut separated = PeerLaneQueue::default();
    separated.enqueue(append(10));
    separated.enqueue(timeout_now(20));
    separated.enqueue(append(30));

    assert_append(separated.dequeue(), 10);
    assert_timeout_now(separated.dequeue(), 20, 7);
    assert_append(separated.dequeue(), 30);
    assert!(separated.dequeue().is_none());
}

#[test]
fn peer_lane_keeps_every_non_append_message_lossless_and_fifo() {
    let mut queue = PeerLaneQueue::default();
    let expected = vec![
        RaftMsg::Vote(VoteReq {
            term: 10,
            candidate: NodeId::new(1),
            last_log_index: 2,
            last_log_term: 3,
        }),
        RaftMsg::VoteResp(VoteResp {
            term: 20,
            granted: true,
        }),
        RaftMsg::AppendResp(AppendResp {
            term: 30,
            success: true,
            match_index: 4,
        }),
        RaftMsg::InstallSnapshot(InstallSnapshotReq {
            term: 40,
            leader: NodeId::new(1),
            snapshot_index: 5,
            snapshot_term: 6,
            data: vec![7],
        }),
        RaftMsg::InstallSnapshotResp(InstallSnapshotResp {
            term: 50,
            accepted: true,
            snapshot_index: 8,
        }),
        timeout_now(60),
        append(70),
    ];
    for message in expected.iter().cloned() {
        queue.enqueue(message);
    }

    for expected_message in expected {
        let actual = queue.dequeue().expect("every queued message is retained");
        assert_same_message(actual, expected_message);
    }
    assert!(queue.dequeue().is_none());
}
