use super::*;
use crate::NodeId;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

use raft_core::{Index, Membership};

use crate::{RaftStateMachine, RaftStore, StateMachineError};

struct CountingSm(AtomicU64);
impl RaftStateMachine for CountingSm {
    fn apply(&self, index: Index, _: &[u8]) -> Result<(), StateMachineError> {
        self.0.store(index.get(), Ordering::Release);
        Ok(())
    }
    fn snapshot(&self, writer: &mut dyn Write) -> Result<(), StateMachineError> {
        writer
            .write_all(&self.0.load(Ordering::Acquire).to_le_bytes())
            .map_err(StateMachineError::other)?;
        Ok(())
    }
    fn restore(&self, reader: &mut dyn Read) -> Result<(), StateMachineError> {
        let mut bytes = [0; 8];
        reader
            .read_exact(&mut bytes)
            .map_err(StateMachineError::other)?;
        self.0.store(u64::from_le_bytes(bytes), Ordering::Release);
        Ok(())
    }
    fn applied_index(&self) -> Index {
        Index::new(self.0.load(Ordering::Acquire))
    }
}

#[test]
fn single_voter_is_deterministic_and_persists_before_reopen() {
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        crate::FsyncPolicy::Always,
    )
    .unwrap();
    let sm = Arc::new(CountingSm(AtomicU64::new(0)));
    let mut host = DeterministicHost::open(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0)], vec![]),
        store,
        sm.clone(),
    )
    .unwrap();
    for _ in 0..50 {
        host.tick().unwrap();
    }
    assert_eq!(host.view().role, ConformanceRole::Leader);
    assert_eq!(host.try_propose(vec![7].into()).unwrap(), Index::new(1));
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        crate::FsyncPolicy::Always,
    )
    .unwrap();
    drop(host);
    let after = Arc::new(CountingSm(AtomicU64::new(0)));
    let reopened = DeterministicHost::open(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(9)], vec![]),
        store,
        after.clone(),
    )
    .unwrap();
    assert_eq!(after.applied_index(), Index::new(1));
    assert_eq!(reopened.view().membership.voters(), vec![NodeId::new(0)]);
}

#[test]
fn nonleaders_report_not_leader_for_all_admission_paths() {
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(1),
        crate::FsyncPolicy::Always,
    )
    .unwrap();
    let sm = Arc::new(CountingSm(AtomicU64::new(0)));
    let mut host = DeterministicHost::open(
        NodeId::new(1),
        Membership::new(vec![NodeId::new(0), NodeId::new(1), NodeId::new(2)], vec![]),
        store,
        sm,
    )
    .unwrap();
    assert!(matches!(
        host.try_propose(vec![1].into()),
        Err(StepError::NotLeader)
    ));
    assert!(matches!(
        host.try_add_learner(NodeId::new(3)),
        Err(StepError::NotLeader)
    ));
    assert!(matches!(
        host.promote_learner(NodeId::new(3)),
        Err(StepError::NotLeader)
    ));
    assert!(matches!(
        host.demote_voter(NodeId::new(0)),
        Err(StepError::NotLeader)
    ));
    assert!(matches!(
        host.remove_member(NodeId::new(0)),
        Err(StepError::NotLeader)
    ));
}
