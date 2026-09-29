use super::*;
use raft_core::{AppendReq, VoteResp};

use crate::application::host::apply::apply_ready_with_admission;
use crate::interfaces::peer_http::host_status;

struct TestPermit {
    id: u64,
    drops: Arc<AtomicU64>,
}

impl Drop for TestPermit {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct AdmissionSm {
    reject: Option<ProposalBackpressure>,
    next_permit: AtomicU64,
    admitted: AtomicU64,
    applied: AtomicU64,
    drops: Arc<AtomicU64>,
    gate_entered: AtomicBool,
    gate_release: AtomicBool,
    early_watermark_then_fail: AtomicBool,
}

impl AdmissionSm {
    fn accepting() -> Arc<Self> {
        Arc::new(Self {
            reject: None,
            next_permit: AtomicU64::new(0),
            admitted: AtomicU64::new(0),
            applied: AtomicU64::new(0),
            drops: Arc::new(AtomicU64::new(0)),
            gate_entered: AtomicBool::new(false),
            gate_release: AtomicBool::new(true),
            early_watermark_then_fail: AtomicBool::new(false),
        })
    }

    fn rejecting(reason: &str, retry_after_seconds: u64) -> Arc<Self> {
        Arc::new(Self {
            reject: Some(ProposalBackpressure {
                reason: reason.to_string(),
                retry_after_seconds,
            }),
            next_permit: AtomicU64::new(0),
            admitted: AtomicU64::new(0),
            applied: AtomicU64::new(0),
            drops: Arc::new(AtomicU64::new(0)),
            gate_entered: AtomicBool::new(false),
            gate_release: AtomicBool::new(true),
            early_watermark_then_fail: AtomicBool::new(false),
        })
    }
}

impl RaftStateMachine for AdmissionSm {
    fn admit_proposal(&self, _command: &[u8]) -> anyhow::Result<Option<AdmissionPermit>> {
        if let Some(backpressure) = &self.reject {
            return Err(anyhow::Error::new(backpressure.clone()));
        }
        let id = self.next_permit.fetch_add(1, Ordering::SeqCst) + 1;
        self.admitted.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Box::new(TestPermit {
            id,
            drops: self.drops.clone(),
        })))
    }

    fn apply_admitted(
        &self,
        index: Index,
        _command: &[u8],
        permit: Option<AdmissionPermit>,
    ) -> anyhow::Result<()> {
        let permit =
            permit.map(|permit| permit.downcast::<TestPermit>().expect("test permit type"));
        if let Some(permit) = &permit {
            assert_eq!(permit.id, index, "permit identity follows Raft index");
        }
        if self.early_watermark_then_fail.load(Ordering::SeqCst) {
            self.applied.store(index, Ordering::SeqCst);
        }
        self.gate_entered.store(true, Ordering::SeqCst);
        while !self.gate_release.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        if self.early_watermark_then_fail.load(Ordering::SeqCst) {
            anyhow::bail!("injected error after an early state-machine watermark");
        }
        self.applied.store(index, Ordering::SeqCst);
        drop(permit);
        Ok(())
    }

    fn apply(&self, index: Index, _command: &[u8]) -> anyhow::Result<()> {
        self.applied.store(index, Ordering::SeqCst);
        Ok(())
    }
    fn snapshot(&self, _writer: &mut dyn Write) -> anyhow::Result<()> {
        Ok(())
    }
    fn restore(&self, _reader: &mut dyn Read) -> anyhow::Result<()> {
        Ok(())
    }
    fn applied_index(&self) -> Index {
        self.applied.load(Ordering::SeqCst)
    }
}

struct AdmissionTestHost {
    host: RaftHost,
    _directory: tempfile::TempDir,
}
impl std::ops::Deref for AdmissionTestHost {
    type Target = RaftHost;
    fn deref(&self) -> &Self::Target {
        &self.host
    }
}
async fn elected_single_host(sm: Arc<AdmissionSm>) -> AdmissionTestHost {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    let host = RaftHost::spawn(
        0,
        Membership {
            voters: vec![0],
            learners: vec![],
        },
        HashMap::new(),
        RaftStore::open(path.to_str().unwrap(), 0, crate::FsyncPolicy::Os).unwrap(),
        sm as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    );
    {
        let mut node = host.shared.node.lock().await;
        for _ in 0..raft_core::ELECTION_TIMEOUT_FLOOR_TICKS {
            node.tick();
        }
    }
    AdmissionTestHost {
        host,
        _directory: dir,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proposal_ack_waits_for_callback_completion_not_an_early_watermark() {
    struct ReleaseGate(Arc<AdmissionSm>);
    impl Drop for ReleaseGate {
        fn drop(&mut self) {
            self.0.gate_release.store(true, Ordering::SeqCst);
        }
    }
    let sm = AdmissionSm::accepting();
    let host = Arc::new(elected_single_host(sm.clone()).await);
    sm.early_watermark_then_fail.store(true, Ordering::SeqCst);
    sm.gate_release.store(false, Ordering::SeqCst);
    let release = ReleaseGate(sm.clone());
    let submitting = host.clone();
    let submit = tokio::spawn(async move { submitting.propose_outcome(b"early".to_vec()).await });
    let entered = tokio::time::timeout(Duration::from_secs(2), async {
        while !sm.gate_entered.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let before = tokio::time::timeout(Duration::from_secs(2), host_status(&host.shared)).await;
    // Give the public waiter a chance to observe the premature SM watermark.
    tokio::time::sleep(Duration::from_millis(30)).await;
    let acknowledged_while_callback_blocked = submit.is_finished();
    drop(release);
    let joined = tokio::time::timeout(Duration::from_secs(2), submit).await;
    let after = tokio::time::timeout(Duration::from_secs(2), host_status(&host.shared)).await;
    assert!(
        entered.is_ok(),
        "apply callback must reach the early watermark gate"
    );
    assert!(
        !acknowledged_while_callback_blocked,
        "public proposal returned before apply callback completed"
    );
    assert_eq!(
        before.unwrap().applied_index,
        0,
        "public applied head requires callback completion"
    );
    assert!(matches!(
        joined.unwrap().unwrap(),
        ProposalOutcome::DurabilityFailure { index: Some(1), .. }
    ));
    assert_eq!(
        after.unwrap().applied_index,
        0,
        "failed callback must not publish an applied head"
    );
}

#[tokio::test]
async fn proposal_admission_rejects_before_raft_append() {
    let sm = AdmissionSm::rejecting("test budget full", 7);
    let host = elected_single_host(sm).await;
    let error = host.propose(b"full".to_vec()).await.unwrap_err();
    let backpressure = error.downcast_ref::<ProposalBackpressure>().unwrap();
    assert_eq!(backpressure.retry_after_seconds, 7);
    assert_eq!(host.shared.node.lock().await.last_index(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn indexed_permit_survives_cancelled_producer_until_apply_returns() {
    struct ReleaseGate(Arc<AdmissionSm>);
    impl Drop for ReleaseGate {
        fn drop(&mut self) {
            self.0.gate_release.store(true, Ordering::SeqCst);
        }
    }
    let sm = AdmissionSm::accepting();
    sm.gate_release.store(false, Ordering::SeqCst);
    let release = ReleaseGate(sm.clone());
    let host = Arc::new(elected_single_host(sm.clone()).await);
    let mut submit = tokio::spawn({
        let host = host.clone();
        async move { host.propose(vec![1]).await }
    });
    let entered = tokio::time::timeout(Duration::from_secs(2), async {
        while !sm.gate_entered.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let before_cancel = sm.drops.load(Ordering::SeqCst);
    submit.abort();
    let after_cancel = sm.drops.load(Ordering::SeqCst);
    // Release the worker before any assertion or join, including failures.
    drop(release);
    let joined = tokio::time::timeout(Duration::from_secs(2), &mut submit).await;
    let applied = tokio::time::timeout(Duration::from_secs(2), async {
        // The SM floor can precede callback return and permit drop.
        // Observe the host publication, which follows both.
        while host.shared.completed_applied_index() < 1 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(entered.is_ok(), "apply did not reach the gate");
    assert!(
        joined.is_ok(),
        "cancelled producer did not finish after release"
    );
    assert!(
        applied.is_ok(),
        "committed apply did not finish after release"
    );
    assert_eq!(before_cancel, 0, "permit must reach the apply callback");
    assert_eq!(after_cancel, 0, "cancellation cannot drop the apply permit");
    assert_eq!(sm.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn identical_commands_get_distinct_index_permits() {
    let sm = AdmissionSm::accepting();
    let host = elected_single_host(sm.clone()).await;
    assert_eq!(host.propose(vec![9]).await.unwrap(), 1);
    assert_eq!(host.propose(vec![9]).await.unwrap(), 2);
    assert_eq!(sm.admitted.load(Ordering::SeqCst), 2);
    assert_eq!(sm.drops.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn persistence_failure_after_index_keeps_the_host_owned_permit() {
    let sm = AdmissionSm::accepting();
    let host = elected_single_host(sm.clone()).await;
    host.shared
        .store
        .inject_next_save_failure_with_kind(std::io::ErrorKind::Other);
    assert!(host.propose(vec![4]).await.is_err());
    assert_eq!(host.shared.node.lock().await.last_index(), 1);
    assert_eq!(
        host.shared.pending_admission.lock().unwrap().len(),
        1,
        "the appended index still owns its permit after uncertain persistence"
    );
    assert_eq!(sm.drops.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn forwarded_backpressure_reply_restores_typed_error_data() {
    let leader = elected_single_host(AdmissionSm::rejecting("leader full", 11)).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, leader.router()).await.unwrap();
    });
    let client = elected_single_host(AdmissionSm::accepting()).await;
    let outcome = client
        .forward(&format!("http://{address}"), b"forwarded")
        .await;
    server.abort();
    let ProposalOutcome::RejectedBeforeAdmission { reason } = outcome else {
        panic!("forwarded 429 must remain a pre-append rejection");
    };
    assert_eq!(
        decode_backpressure(&reason),
        Some(ProposalBackpressure {
            reason: "leader full".to_string(),
            retry_after_seconds: 11,
        })
    );
}

#[test]
fn conflicting_term_at_same_index_drops_only_replaced_uncommitted_permit() {
    let membership = Membership {
        voters: vec![0, 1],
        learners: vec![],
    };
    let mut node = RaftNode::new(0, &membership);
    for _ in 0..raft_core::ELECTION_TIMEOUT_FLOOR_TICKS {
        node.tick();
    }
    node.handle(
        1,
        RaftMsg::VoteResp(VoteResp {
            term: 1,
            granted: true,
        }),
    );
    let index = node.propose(vec![1]).unwrap();
    let old_term = node.current_term();
    assert_eq!(index, 1);
    node.handle(
        1,
        RaftMsg::Append(AppendReq {
            term: old_term + 1,
            leader: 1,
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![raft_core::RaftEntry {
                term: old_term + 1,
                index,
                command: vec![2],
                kind: raft_core::EntryKind::Command,
            }],
            leader_commit: index,
        }),
    );
    let sm = AdmissionSm::accepting();
    let drops = Arc::new(AtomicU64::new(0));
    let pending = StdMutex::new(BTreeMap::from([(
        (index, old_term),
        Box::new(TestPermit {
            id: 99,
            drops: drops.clone(),
        }) as AdmissionPermit,
    )]));
    apply_ready_with_admission(
        &mut node,
        sm.as_ref(),
        None,
        SnapshotPolicy::Disabled,
        true,
        Some(&pending),
    )
    .unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(pending.lock().unwrap().is_empty());
    assert_eq!(sm.applied_index(), index);
}
