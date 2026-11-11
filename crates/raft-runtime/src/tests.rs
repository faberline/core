use super::*;
use crate::StateMachineError;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod peer_wire_golden;
mod peer_wire_split;

/// A trivial state machine: applies `u64` commands into a log, tracks the
/// applied index, snapshots/restores the whole log.
struct CounterSm {
    log: Mutex<Vec<(Index, u64)>>,
    applied: AtomicU64,
    apply_mode: AtomicU64,
}
impl CounterSm {
    fn new() -> Arc<Self> {
        Arc::new(CounterSm {
            log: Mutex::new(Vec::new()),
            applied: AtomicU64::new(0),
            apply_mode: AtomicU64::new(0),
        })
    }
}
impl RaftStateMachine for CounterSm {
    fn apply(&self, index: Index, command: &[u8]) -> Result<(), StateMachineError> {
        match self.apply_mode.load(Ordering::Acquire) {
            1 => {
                self.applied.store(index.get(), Ordering::Release);
                return Err(StateMachineError::other(
                    "injected completed domain refusal",
                ));
            }
            2 => {
                return Err(StateMachineError::other(
                    "injected incomplete infrastructure failure",
                ))
            }
            _ => {}
        }
        let v = u64::from_le_bytes(command.try_into().unwrap_or([0; 8]));
        self.log.lock().unwrap().push((index, v));
        self.applied.store(index.get(), Ordering::Release);
        Ok(())
    }
    fn snapshot(&self, writer: &mut dyn Write) -> Result<(), StateMachineError> {
        let bytes =
            serde_json::to_vec(&*self.log.lock().unwrap()).map_err(StateMachineError::other)?;
        writer.write_all(&bytes).map_err(StateMachineError::other)?;
        Ok(())
    }
    fn restore(&self, reader: &mut dyn Read) -> Result<(), StateMachineError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(StateMachineError::other)?;
        let log: Vec<(Index, u64)> =
            serde_json::from_slice(&bytes).map_err(StateMachineError::other)?;
        let last = log.last().map(|(i, _)| *i).unwrap_or(Index::new(0));
        *self.log.lock().unwrap() = log;
        self.applied.store(last.get(), Ordering::Release);
        Ok(())
    }
    fn applied_index(&self) -> Index {
        Index::new(self.applied.load(Ordering::Acquire))
    }
}

fn store(dir: &std::path::Path, id: NodeId) -> RaftStore {
    RaftStore::open(dir.to_str().unwrap(), id, FsyncPolicy::Os).unwrap()
}

#[test]
fn default_apply_admitted_preserves_completed_domain_error() {
    let sm = CounterSm::new();
    sm.apply_mode.store(1, Ordering::Release);

    RaftStateMachine::apply_admitted(&*sm, Index::new(7), &7_u64.to_le_bytes(), None).unwrap();
    assert_eq!(sm.applied_index(), Index::new(7));
}

#[test]
fn default_apply_admitted_preserves_incomplete_infrastructure_error() {
    let sm = CounterSm::new();
    sm.apply_mode.store(2, Ordering::Release);

    let error = RaftStateMachine::apply_admitted(&*sm, Index::new(7), &7_u64.to_le_bytes(), None)
        .expect_err("an unchanged floor must preserve the apply error");
    assert!(error.to_string().contains("incomplete infrastructure"));
    assert_eq!(sm.applied_index(), Index::new(0));
}

/// Waits until `host` is leader and returns the last index it holds then.
/// With `leader_noop` on, that index is the leader's no-op entry.
async fn wait_leader_base(host: &RaftHost) -> u64 {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            {
                let node = host.shared.node.lock().await;
                if node.is_leader() {
                    return node.last_index().get();
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("host became leader")
}

#[tokio::test]
async fn single_node_propose_applies_read_your_write() {
    let tmp = std::env::temp_dir().join(format!("raft-runtime-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let sm = CounterSm::new();
    let host = RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0)], vec![]),
        std::collections::HashMap::new(),
        store(&tmp, NodeId::new(0)),
        sm.clone() as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    );
    let base = wait_leader_base(&host).await;
    // propose returns only after the SM has applied the entry (RYW).
    for v in 1..=3u64 {
        let idx = host.propose(v.to_le_bytes().to_vec()).await.unwrap();
        assert_eq!(idx, Index::new(base + v));
        assert!(sm.applied_index() >= idx, "applied before propose returned");
    }
    let log = sm.log.lock().unwrap().clone();
    assert_eq!(
        log,
        vec![
            (Index::new(base + 1), 1),
            (Index::new(base + 2), 2),
            (Index::new(base + 3), 3)
        ]
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn restart_replays_committed_log_into_a_fresh_sm() {
    let tmp = std::env::temp_dir().join(format!("raft-runtime-replay-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    // Use Always fsync so the log is durable across the "restart".
    let mk = |sm: Arc<dyn RaftStateMachine>| {
        RaftHost::spawn(
            NodeId::new(0),
            Membership::new(vec![NodeId::new(0)], vec![]),
            std::collections::HashMap::new(),
            RaftStore::open(tmp.to_str().unwrap(), NodeId::new(0), FsyncPolicy::Always).unwrap(),
            sm,
            HostConfig::default(),
        )
    };
    let base1;
    {
        let sm = CounterSm::new();
        let host = mk(sm.clone());
        base1 = wait_leader_base(&host).await;
        host.propose(7u64.to_le_bytes().to_vec()).await.unwrap();
        host.propose(8u64.to_le_bytes().to_vec()).await.unwrap();
        assert_eq!(sm.applied_index(), Index::new(base1 + 2));
    } // host dropped → tasks aborted (simulated restart)
      // The persisted commit watermark lets a fresh in-memory state machine
      // replay before it serves reads or accepts another proposal.
    let sm2 = CounterSm::new();
    let host2 = mk(sm2.clone());
    assert_eq!(sm2.applied_index(), Index::new(base1 + 2));
    assert_eq!(
        sm2.log.lock().unwrap().clone(),
        vec![(Index::new(base1 + 1), 7), (Index::new(base1 + 2), 8)]
    );
    // The new term appends its own no-op, so read the base after the restart.
    let base2 = wait_leader_base(&host2).await;
    let idx = host2.propose(9u64.to_le_bytes().to_vec()).await.unwrap();
    assert_eq!(idx, Index::new(base2 + 1));
    assert_eq!(sm2.applied_index(), Index::new(base2 + 1));
    assert_eq!(
        sm2.log.lock().unwrap().clone(),
        vec![
            (Index::new(base1 + 1), 7),
            (Index::new(base1 + 2), 8),
            (Index::new(base2 + 1), 9)
        ],
        "the cold replay and new command remain in order"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
