//! Proposal outcome typing and classification tests (#3663, #3665).
//!
//! # What was missing
//!
//! Every failure of `RaftHost::propose` was an `anyhow::Error` distinguished only
//! by its message string. Twelve terminal failure arms collapsed into it, erasing
//! the critical distinction between commands safe to re-route (never appended)
//! and commands with allocated indices that must not be blindly retried.
//!
//! These rows test:
//! 1. A single-voter proposal succeeds and returns `Completed` with the applied index.
//! 2. A quiesced host returns `RejectedBeforeAdmission` while `propose` returns an error
//!    containing `raft: proposal admission closed`.
//! 3. A host in a group with no leader elected returns `RejectedBeforeAdmission`.
//! 4. A leader with an injected save failure returns `DurabilityFailure` with `Some(index)`.
//! 5. A latched host with prior durability failure returns `DurabilityFailure` with `None`.
//! 6. A follower forwarding to a healthy leader returns `Completed` after its own apply.
//! 7. A follower forwarding to a quiesced leader returns `RejectedBeforeAdmission`.
//! 8. A forwarded request that times out returns an `Ambiguous` routing-timeout outcome.
//! 9. Negative control: sequential proposals on a healthy host strictly increase indices.

use raft_runtime::NodeId;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;

use axum::{http::StatusCode, routing::post, Json, Router};
use raft_core::{AppendReq, EntryKind, RaftEntry, Term};
use raft_runtime::{
    FsyncPolicy, HostConfig, Index, Membership, ProposalOutcome, RaftHost, RaftStateMachine,
    RaftStore,
};
use tempfile::TempDir;
use tokio::io::AsyncReadExt;

use crate::support::cluster;
use cluster::{await_leader, await_leader_with_tick, bind, cluster, peers_excluding, Node, TestSm};

async fn custom_timeout_cluster(n: u64, timeout: Duration) -> (Vec<Node>, Duration) {
    let mut listeners = Vec::new();
    let mut all = Vec::new();
    for id in 0..n {
        let (l, url) = bind().await;
        listeners.push(l);
        all.push((NodeId::new(id), url));
    }
    let voters: Vec<NodeId> = (0..n).map(NodeId::new).collect();
    let cfg = HostConfig::default().with_propose_timeout(timeout);
    let mut nodes = Vec::new();
    for (idx, listener) in listeners.into_iter().enumerate() {
        let id = NodeId::new(idx as u64);
        let peers = peers_excluding(id, &all);
        let sm = TestSm::new();
        let dir = TempDir::new().unwrap();
        let store = RaftStore::open(dir.path().to_str().unwrap(), id, FsyncPolicy::Os).unwrap();
        let host = Arc::new(RaftHost::spawn(
            id,
            Membership::new(voters.clone(), vec![]),
            peers,
            store,
            sm.clone() as Arc<dyn RaftStateMachine>,
            cfg,
        ));
        let router = host.router();
        let serve = tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let r = router.clone();
                    tokio::spawn(async move {
                        let _ = transport_h2c::server::serve_connection(stream, r).await;
                    });
                }
            }
        });
        let url = all[idx].1.clone();
        nodes.push(Node {
            host,
            sm,
            url,
            _serve: serve,
            _dir: dir,
        });
    }
    (nodes, cfg.tick())
}

async fn publish_response_stub(status: StatusCode, body: serde_json::Value) -> String {
    let (listener, url) = bind().await;
    let router = Router::new().route(
        "/raft/publish",
        post(move || {
            let body = body.clone();
            async move { (status, Json(body)) }
        }),
    );
    tokio::spawn(async move {
        let (stream, _) = listener
            .accept()
            .await
            .expect("the publish-response stub accepts a forwarded request");
        transport_h2c::server::serve_connection(stream, router)
            .await
            .expect("the publish-response stub serves the forwarded request");
    });
    url
}

/// An applied single-voter proposal reports Completed with the index the state machine applied.
#[tokio::test]
async fn single_voter_completed_outcome_matches_applied_index() {
    let nodes = cluster(1).await;
    let outcome = nodes[0]
        .host
        .propose_outcome(b"single-voter-cmd".to_vec())
        .await;

    match outcome {
        ProposalOutcome::Completed { index } => {
            assert!(
                index >= Index::new(1),
                "completed index must be >= 1, got {index}"
            );
            assert_eq!(
                nodes[0].sm.applied_index(),
                index,
                "state machine applied index must equal outcome index"
            );
        }
        other => panic!("expected ProposalOutcome::Completed, got: {other:?}"),
    }
}

/// A quiesced host returns RejectedBeforeAdmission, and propose() on that same host
/// returns an Err containing "proposal admission closed".
#[tokio::test]
async fn quiesced_host_rejects_before_admission_and_propose_preserves_error_string() {
    let nodes = cluster(1).await;
    let quiesced = nodes[0].host.quiesce_proposals();
    assert!(
        quiesced,
        "the first quiesce_proposals call must return true"
    );

    let outcome = nodes[0]
        .host
        .propose_outcome(b"cmd-after-quiesce".to_vec())
        .await;
    match outcome {
        ProposalOutcome::RejectedBeforeAdmission { ref reason } => {
            assert!(
                reason.contains("proposal admission closed"),
                "reason must contain 'proposal admission closed', got: {reason}"
            );
        }
        other => panic!("expected ProposalOutcome::RejectedBeforeAdmission, got: {other:?}"),
    }

    let res = nodes[0].host.propose(b"cmd-propose".to_vec()).await;
    assert!(res.is_err(), "propose() on quiesced host must return Err");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("proposal admission closed"),
        "error message must contain 'proposal admission closed', got: {err_msg}"
    );
}

/// A host that never wins an election reports RejectedBeforeAdmission rather than
/// Ambiguous, because nothing was ever sent.
#[tokio::test]
async fn no_leader_elected_rejects_before_admission() {
    let sm = TestSm::new();
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Os,
    )
    .unwrap();
    let cfg = HostConfig::default().with_propose_timeout(Duration::from_millis(150));
    let host = RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0), NodeId::new(1)], vec![]),
        HashMap::new(),
        store,
        sm.clone() as Arc<dyn RaftStateMachine>,
        cfg,
    );

    let outcome = host.propose_outcome(b"cmd-no-leader".to_vec()).await;
    match outcome {
        ProposalOutcome::RejectedBeforeAdmission { reason } => {
            assert!(
                reason.contains("no leader elected"),
                "reason must contain 'no leader elected', got: {reason}"
            );
        }
        other => panic!("expected ProposalOutcome::RejectedBeforeAdmission, got: {other:?}"),
    }
}

/// A single-voter leader with an injected save failure reports DurabilityFailure
/// carrying Some(index).
#[tokio::test]
async fn injected_save_failure_reports_durability_failure_with_allocated_index() {
    let nodes = cluster(1).await;
    nodes[0]
        .host
        .store()
        .inject_next_save_failure_with_kind(ErrorKind::StorageFull);

    let outcome = nodes[0]
        .host
        .propose_outcome(b"cmd-save-failure".to_vec())
        .await;
    match outcome {
        ProposalOutcome::DurabilityFailure { index, failure } => {
            assert!(
                index.is_some(),
                "first durability failure must have allocated index Some(i)"
            );
            let idx = index.unwrap();
            assert!(
                idx >= Index::new(1),
                "allocated index must be >= 1, got {idx}"
            );
            assert_eq!(failure.kind, ErrorKind::StorageFull);
        }
        other => {
            panic!("expected ProposalOutcome::DurabilityFailure with Some(index), got: {other:?}")
        }
    }
}

/// A latched host with prior durability failure reports DurabilityFailure carrying None,
/// because the latched check runs before any append.
#[tokio::test]
async fn latched_durability_failure_reports_none_index() {
    let nodes = cluster(1).await;
    nodes[0]
        .host
        .store()
        .inject_next_save_failure_with_kind(ErrorKind::StorageFull);

    // First proposal triggers save failure and latches failure state.
    let outcome1 = nodes[0]
        .host
        .propose_outcome(b"cmd-first-save-failure".to_vec())
        .await;
    match outcome1 {
        ProposalOutcome::DurabilityFailure { index, .. } => {
            assert!(index.is_some(), "first proposal must have allocated index");
        }
        other => panic!("expected first proposal to fail with DurabilityFailure, got: {other:?}"),
    }

    // Second proposal on the same host without re-arming injector.
    let outcome2 = nodes[0]
        .host
        .propose_outcome(b"cmd-latched-proposal".to_vec())
        .await;
    match outcome2 {
        ProposalOutcome::DurabilityFailure { index, failure } => {
            assert!(
                index.is_none(),
                "latched durability failure must have index None, got {index:?}"
            );
            assert_eq!(failure.kind, ErrorKind::StorageFull);
        }
        other => panic!("expected ProposalOutcome::DurabilityFailure with None, got: {other:?}"),
    }
}

/// A node known to be a follower forwards to the live leader and returns the
/// completed index only after that follower applies the command itself.
#[tokio::test]
async fn follower_forwarding_to_live_leader_completes_after_local_apply() {
    let nodes = cluster(3).await;
    let leader = await_leader(&nodes)
        .await
        .expect("a three-voter cluster elects a leader");
    let follower = (leader + 1) % 3;
    assert!(
        !nodes[follower].host.is_leader().await,
        "selected node must be a follower before it forwards"
    );

    let outcome = nodes[follower]
        .host
        .propose_outcome(b"forwarded-cmd-to-live-leader".to_vec())
        .await;

    match outcome {
        ProposalOutcome::Completed { index } => {
            assert!(
                index >= Index::new(1),
                "forwarded completed index must be positive"
            );
            assert_eq!(
                nodes[follower].sm.applied_index(),
                index,
                "the forwarding follower must apply the completed index"
            );
            assert_eq!(
                nodes[leader].sm.applied_index(),
                index,
                "the original leader must commit the forwarded command"
            );
        }
        other => panic!("expected forwarded ProposalOutcome::Completed, got: {other:?}"),
    }
}

/// A follower forwarding to a quiesced leader preserves the leader's typed
/// rejection instead of treating a known pre-append refusal as ambiguous.
#[tokio::test]
async fn follower_forwarding_to_quiesced_leader_reports_rejected_before_admission() {
    let (nodes, tick) = custom_timeout_cluster(3, Duration::from_millis(300)).await;
    let leader = await_leader_with_tick(&nodes, tick)
        .await
        .expect("a three-voter cluster elects a leader");

    let quiesced = nodes[leader].host.quiesce_proposals();
    assert!(
        quiesced,
        "the first quiesce_proposals call must return true"
    );

    let follower = (leader + 1) % 3;
    let outcome = nodes[follower]
        .host
        .propose_outcome(b"forwarded-cmd-to-quiesced-leader".to_vec())
        .await;

    match outcome {
        ProposalOutcome::RejectedBeforeAdmission { reason } => {
            assert!(
                reason.contains("proposal admission closed"),
                "forwarded admission rejection must preserve its reason, got: {reason}"
            );
        }
        other => panic!("expected ProposalOutcome::RejectedBeforeAdmission, got: {other:?}"),
    }
}

/// A registered leader address that accepts h2c but never responds makes the
/// forwarding deadline expire. This is distinct from both a typed admission
/// rejection and a malformed immediate reply.
#[tokio::test]
async fn follower_forwarding_transport_timeout_is_ambiguous_with_routing_timeout_reason() {
    let (nodes, tick) = custom_timeout_cluster(3, Duration::from_millis(300)).await;
    let leader = await_leader_with_tick(&nodes, tick)
        .await
        .expect("a three-voter cluster elects a leader");
    let follower = (leader + 1) % 3;
    assert!(
        !nodes[follower].host.is_leader().await,
        "selected node must be a follower before it forwards"
    );
    let (listener, stalled_url) = bind().await;
    nodes[follower]
        .host
        .upsert_peer(NodeId::new(leader as u64), stalled_url)
        .await;

    let stalled_request = tokio::spawn(async move {
        let (mut stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .expect("the follower sends a forwarded request before its deadline")
            .expect("the stalled listener accepts the forwarded request");
        let mut preface = [0_u8; 24];
        tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut preface))
            .await
            .expect("the forwarding client writes the h2c preface")
            .expect("the forwarding client writes a complete h2c preface");
        assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
        tokio::time::sleep(Duration::from_millis(600)).await;
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(2),
        nodes[follower]
            .host
            .propose_outcome(b"forwarded-cmd-to-stalled-leader".to_vec()),
    )
    .await
    .expect("the forwarding proposal respects its deadline");
    tokio::time::timeout(Duration::from_secs(2), stalled_request)
        .await
        .expect("the stalled listener completes")
        .expect("the stalled listener does not panic");

    match outcome {
        ProposalOutcome::Ambiguous {
            index: None,
            reason,
        } => {
            assert!(
                reason.contains("proposal routing timed out"),
                "transport timeout must use the routing-timeout outcome, got: {reason}"
            );
        }
        other => panic!("expected routing-timeout ProposalOutcome::Ambiguous, got: {other:?}"),
    }
}

/// A 503 is typed only when it contains both the exact outcome discriminator
/// and a string refusal reason. A malformed admission body remains ambiguous.
#[tokio::test]
async fn follower_forwarding_malformed_admission_body_remains_ambiguous() {
    let (nodes, tick) = custom_timeout_cluster(3, Duration::from_millis(300)).await;
    let leader = await_leader_with_tick(&nodes, tick)
        .await
        .expect("a three-voter cluster elects a leader");
    let follower = (leader + 1) % 3;
    let malformed_url = publish_response_stub(
        StatusCode::SERVICE_UNAVAILABLE,
        serde_json::json!({ "outcome": "rejected_before_admission" }),
    )
    .await;
    nodes[follower]
        .host
        .upsert_peer(NodeId::new(leader as u64), malformed_url)
        .await;

    let outcome = nodes[follower]
        .host
        .propose_outcome(b"forwarded-cmd-to-malformed-admission-reply".to_vec())
        .await;

    match outcome {
        ProposalOutcome::Ambiguous { index: None, .. } => {}
        other => panic!("expected malformed forwarded refusal to remain Ambiguous, got: {other:?}"),
    }
}

/// Truncating a proposal's uncommitted entry through a leader change must not report Completed.
///
/// In a multi-voter cluster, a proposal on node 0 is appended at (index, term 1) but
/// remains uncommitted without peer quorum. When node 1 takes over as leader in term 2
/// and replicates a conflicting entry that commits at that same index, node 0 truncates
/// its uncommitted entry and applies the replacing entry. The proposer must receive
/// Ambiguous naming replacing term 2, not Completed.
#[tokio::test]
async fn uncommitted_proposal_superseded_by_leader_change_returns_ambiguous() {
    use raft_core::VoteResp;

    let sm = TestSm::new();
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Os,
    )
    .unwrap();
    let cfg = HostConfig::default().with_propose_timeout(Duration::from_secs(2));
    let host = Arc::new(RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0), NodeId::new(1)], vec![]),
        HashMap::new(),
        store,
        sm.clone() as Arc<dyn RaftStateMachine>,
        cfg,
    ));

    // Wait until node 0 campaigns, then grant voter 1's vote so node 0 becomes leader.
    let start = std::time::Instant::now();
    loop {
        let st = host.status().await;
        host.step_vote_response(
            NodeId::new(1),
            VoteResp {
                term: Term::new(st.term),
                granted: true,
            },
        )
        .await;
        if host.is_leader().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        if start.elapsed() > Duration::from_secs(2) {
            panic!("node 0 did not become leader within deadline");
        }
    }

    // Propose on leader node 0. Because voter 1 has not acknowledged it,
    // the proposal is appended at index 1 (term 1) but cannot commit.
    let host_clone = Arc::clone(&host);
    let propose_task = tokio::spawn(async move {
        host_clone
            .propose_outcome(b"superseded-command".to_vec())
            .await
    });

    // Give the proposal a moment to be appended and enter the apply-wait loop.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Leader change: node 1 takes over in term 2 and replicates a conflicting entry
    // that commits at index 1, truncating node 0's uncommitted proposal.
    let supersede_req = AppendReq {
        term: Term::new(2),
        leader: NodeId::new(1),
        prev_log_index: Index::new(0),
        prev_log_term: Term::new(0),
        entries: vec![RaftEntry {
            term: Term::new(2),
            index: Index::new(1),
            command: vec![42],
            kind: EntryKind::Command,
        }],
        leader_commit: Index::new(1),
    };

    let resp = host.step_append_entries(NodeId::new(1), supersede_req).await;
    assert!(resp.success, "append entries must succeed");

    // The proposer must receive Ambiguous (not Completed) stating the entry was superseded
    // and naming the replacing term (2), without waiting for the propose timeout.
    let outcome = tokio::time::timeout(Duration::from_secs(1), propose_task)
        .await
        .expect("proposer must receive outcome promptly without waiting for propose_timeout")
        .expect("propose task did not panic");

    match outcome {
        ProposalOutcome::Ambiguous { index, reason } => {
            assert_eq!(
                index,
                Some(Index::new(1)),
                "ambiguous outcome must identify the proposal index"
            );
            assert!(
                reason.contains("superseded"),
                "reason must indicate entry was superseded, got: {reason}"
            );
            assert!(
                reason.contains("2"),
                "reason must name replacing term (2), got: {reason}"
            );
        }
        ProposalOutcome::Completed { index } => {
            panic!("proposer must NOT receive Completed for superseded entry at index {index}");
        }
        other => panic!("expected Ambiguous outcome for superseded proposal, got: {other:?}"),
    }
}

/// Negative control: sequential successful proposals on a healthy host strictly increase
/// indices and never return Ambiguous, RejectedBeforeAdmission, or DurabilityFailure.
#[tokio::test]
async fn sequential_successful_proposals_increase_indices() {
    let nodes = cluster(1).await;
    let mut prev_index = Index::new(0);

    for i in 1..=4u8 {
        let outcome = nodes[0]
            .host
            .propose_outcome(vec![i, i.wrapping_add(1), 42])
            .await;
        match outcome {
            ProposalOutcome::Completed { index } => {
                assert!(
                    index > prev_index,
                    "indices must strictly increase: {index} > {prev_index}"
                );
                prev_index = index;
                assert_eq!(
                    nodes[0].sm.applied_index(),
                    index,
                    "state machine applied index must equal completed index"
                );
            }
            ProposalOutcome::Ambiguous { .. } => {
                panic!("proposal {i} must not return Ambiguous");
            }
            ProposalOutcome::RejectedBeforeAdmission { .. } => {
                panic!("proposal {i} must not return RejectedBeforeAdmission");
            }
            ProposalOutcome::DurabilityFailure { .. } => {
                panic!("proposal {i} must not return DurabilityFailure");
            }
        }
    }
}

/// Keep the proposal future unpolled while a newer leader commits two entries
/// and the ordered applier compacts them. No scheduler timing determines whether
/// the waiter observes the resident log or the compacted prefix.
async fn proposal_outcome_after_compaction(replaced: bool) -> ProposalOutcome {
    use raft_core::VoteResp;
    use raft_runtime::SnapshotPolicy;
    use std::future::{poll_fn, Future};
    use std::task::Poll;

    let sm = TestSm::new();
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Os,
    )
    .unwrap();
    let host = RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0), NodeId::new(1)], vec![]),
        HashMap::new(),
        store,
        sm.clone() as Arc<dyn RaftStateMachine>,
        HostConfig::default()
            .with_propose_timeout(Duration::from_secs(10))
            .with_snapshot(SnapshotPolicy::EveryEntries(2)),
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let status = host.status().await;
            host.step_vote_response(
                NodeId::new(1),
                VoteResp {
                    term: Term::new(status.term),
                    granted: true,
                },
            )
            .await;
            if host.is_leader().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("host elects a leader");
    let original_term = host.status().await.term;
    let command = b"proposal-before-compaction".to_vec();
    let mut proposal = Box::pin(host.propose_outcome(command.clone()));
    poll_fn(|cx| {
        assert!(proposal.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(host.status().await.last_index, 1, "proposal was appended");

    let applied_term = original_term + u64::from(replaced);
    let boundary_term = applied_term + 1;
    let response = host
        .step_append_entries(
            NodeId::new(1),
            AppendReq {
                term: Term::new(boundary_term),
                leader: NodeId::new(1),
                prev_log_index: Index::new(0),
                prev_log_term: Term::new(0),
                entries: vec![
                    RaftEntry {
                        index: Index::new(1),
                        term: Term::new(applied_term),
                        command: if replaced {
                            b"replacement".to_vec()
                        } else {
                            command.clone()
                        },
                        kind: EntryKind::Command,
                    },
                    RaftEntry {
                        index: Index::new(2),
                        term: Term::new(boundary_term),
                        command: b"later-term-command".to_vec(),
                        kind: EntryKind::Command,
                    },
                ],
                leader_commit: Index::new(2),
            },
        )
        .await;
    assert!(response.success);
    tokio::time::timeout(Duration::from_secs(2), async {
        while host.snapshot_index().await < Index::new(2) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("applier compacts both entries before proposal resumes");
    assert_eq!(sm.applied_index(), Index::new(2));
    let persisted = host.store().load().unwrap().unwrap();
    assert_eq!(persisted.snapshot_term, Term::new(boundary_term));
    assert!(persisted.log.is_empty());
    tokio::time::timeout(Duration::from_secs(1), proposal)
        .await
        .expect("proposal resolves promptly after compaction")
}

#[tokio::test]
async fn applied_proposal_remains_completed_after_later_term_compaction() {
    let outcome = proposal_outcome_after_compaction(false).await;
    assert!(
        matches!(outcome, ProposalOutcome::Completed { index } if index == Index::new(1)),
        "expected original proposal to remain completed, got {outcome:?}"
    );
}

#[tokio::test]
async fn compacted_superseded_proposal_names_actual_replacing_term() {
    match proposal_outcome_after_compaction(true).await {
        ProposalOutcome::Ambiguous { index, reason } => {
            assert_eq!(index, Some(Index::new(1)));
            assert!(reason.contains("superseded by entry of term 2"), "{reason}");
        }
        other => panic!("expected superseded outcome, got {other:?}"),
    }
}
