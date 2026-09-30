//! Errors that cross the state-machine ports keep their type for callers.
//!
//! This copies lumen's pattern (`raft_sm.rs`): `admit_proposal` refuses with a
//! `ProposalBackpressure`, and the caller downcasts the error that
//! `RaftHost::propose` returns to choose a retry. Each row builds the refusal
//! one way an implementor can after the port became `StateMachineError`, and
//! each must reach the caller with its type and fields intact. The snapshot
//! rows cover a port error the host returns directly.

use raft_runtime::NodeId;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use raft_runtime::{
    AdmissionPermit, FsyncPolicy, HostConfig, Index, Membership, MembershipError,
    ProposalBackpressure, RaftHost, RaftStateMachine, RaftStore, StateMachineError,
};
use tempfile::TempDir;

/// A product's own error type, as a caller would downcast it.
#[derive(Debug, PartialEq, Eq)]
struct QuotaExceeded {
    tenant: &'static str,
}

impl std::fmt::Display for QuotaExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tenant {} is over quota", self.tenant)
    }
}

impl std::error::Error for QuotaExceeded {}

/// How the state machine builds its error.
#[derive(Clone, Copy)]
enum Refusal {
    /// Accept every proposal.
    None,
    /// `StateMachineError::other(ProposalBackpressure { .. })`.
    Typed,
    /// lumen today: an `anyhow::Error` from `ProposalBackpressure`, then `?`.
    Anyhow,
    /// An `anyhow::Error` with context, converted with `?`.
    AnyhowWithContext,
    /// An `anyhow::Error` with context, converted with `StateMachineError::other`.
    OtherOfAnyhow,
    /// A product error that is not backpressure.
    Quota,
}

/// How `snapshot` fails.
#[derive(Clone, Copy)]
enum SnapshotFailure {
    None,
    /// An `anyhow::Error` with context, converted with `?`.
    Anyhow,
    /// `StateMachineError::other(QuotaExceeded { .. })`.
    Typed,
}

struct RefusingSm {
    refusal: Refusal,
    snapshot_failure: SnapshotFailure,
    applied: AtomicU64,
}

fn backpressure() -> ProposalBackpressure {
    ProposalBackpressure {
        reason: "record budget full".to_string(),
        retry_after_seconds: 3,
    }
}

fn anyhow_backpressure() -> anyhow::Result<()> {
    Err(anyhow::Error::new(backpressure())).context("admit record")
}

fn anyhow_quota() -> anyhow::Result<()> {
    Err(anyhow::Error::new(QuotaExceeded { tenant: "a" })).context("write snapshot")
}

impl RaftStateMachine for RefusingSm {
    fn admit_proposal(
        &self,
        _command: &[u8],
    ) -> Result<Option<AdmissionPermit>, StateMachineError> {
        match self.refusal {
            Refusal::None => Ok(None),
            Refusal::Typed => Err(StateMachineError::other(backpressure())),
            Refusal::Anyhow => Err(anyhow::Error::new(backpressure()).into()),
            Refusal::AnyhowWithContext => {
                anyhow_backpressure()?;
                Ok(None)
            }
            Refusal::OtherOfAnyhow => {
                anyhow_backpressure().map_err(StateMachineError::other)?;
                Ok(None)
            }
            Refusal::Quota => Err(StateMachineError::other(QuotaExceeded { tenant: "a" })),
        }
    }

    fn apply(&self, index: Index, _command: &[u8]) -> Result<(), StateMachineError> {
        self.applied.store(index, Ordering::Release);
        Ok(())
    }

    fn snapshot(&self, _writer: &mut dyn Write) -> Result<(), StateMachineError> {
        match self.snapshot_failure {
            SnapshotFailure::None => Ok(()),
            SnapshotFailure::Anyhow => {
                anyhow_quota()?;
                Ok(())
            }
            SnapshotFailure::Typed => Err(StateMachineError::other(QuotaExceeded { tenant: "a" })),
        }
    }

    fn restore(&self, _reader: &mut dyn Read) -> Result<(), StateMachineError> {
        Ok(())
    }

    fn applied_index(&self) -> Index {
        self.applied.load(Ordering::Acquire)
    }
}

/// A single-voter host that has elected itself.
async fn leader(refusal: Refusal, snapshot_failure: SnapshotFailure) -> (RaftHost, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let sm = Arc::new(RefusingSm {
        refusal,
        snapshot_failure,
        applied: AtomicU64::new(0),
    });
    let host = RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0)], vec![]),
        Default::default(),
        RaftStore::open(
            dir.path().to_str().unwrap(),
            NodeId::new(0),
            FsyncPolicy::Os,
        )
        .unwrap(),
        sm as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while !host.is_leader().await {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a single voter elects itself");
    (host, dir)
}

async fn refused_proposal(refusal: Refusal) -> anyhow::Error {
    let (host, _dir) = leader(refusal, SnapshotFailure::None).await;
    let error = host.propose(b"record".to_vec()).await.unwrap_err();
    host.shutdown().await.unwrap();
    error
}

#[tokio::test]
async fn typed_backpressure_downcasts_from_propose() {
    let error = refused_proposal(Refusal::Typed).await;
    assert_eq!(
        error.downcast_ref::<ProposalBackpressure>(),
        Some(&backpressure())
    );
}

#[tokio::test]
async fn anyhow_backpressure_downcasts_from_propose() {
    let error = refused_proposal(Refusal::Anyhow).await;
    assert_eq!(
        error.downcast_ref::<ProposalBackpressure>(),
        Some(&backpressure())
    );
}

#[tokio::test]
async fn backpressure_under_anyhow_context_downcasts_from_propose() {
    let error = refused_proposal(Refusal::AnyhowWithContext).await;
    assert_eq!(
        error.downcast_ref::<ProposalBackpressure>(),
        Some(&backpressure())
    );
}

#[tokio::test]
async fn backpressure_under_boxed_anyhow_context_downcasts_from_propose() {
    let error = refused_proposal(Refusal::OtherOfAnyhow).await;
    assert_eq!(
        error.downcast_ref::<ProposalBackpressure>(),
        Some(&backpressure())
    );
}

#[tokio::test]
async fn other_admission_error_keeps_its_text() {
    let error = refused_proposal(Refusal::Quota).await;
    assert_eq!(error.to_string(), "tenant a is over quota");
}

async fn failed_snapshot(snapshot_failure: SnapshotFailure) -> anyhow::Error {
    let (host, _dir) = leader(Refusal::None, snapshot_failure).await;
    host.propose(b"record".to_vec()).await.unwrap();
    let error = host.snapshot_and_compact().await.unwrap_err();
    host.shutdown().await.unwrap();
    error
}

#[tokio::test]
async fn anyhow_snapshot_error_comes_back_whole() {
    let error = failed_snapshot(SnapshotFailure::Anyhow).await;
    assert_eq!(error.to_string(), "write snapshot");
    assert_eq!(
        error.downcast_ref::<QuotaExceeded>(),
        Some(&QuotaExceeded { tenant: "a" })
    );
}

#[tokio::test]
async fn typed_snapshot_error_is_reachable_through_other() {
    let error = failed_snapshot(SnapshotFailure::Typed).await;
    assert_eq!(error.to_string(), "tenant a is over quota");
    let Some(StateMachineError::Other(inner)) = error.downcast_ref::<StateMachineError>() else {
        panic!("a typed port error comes back as StateMachineError::Other: {error:?}");
    };
    assert_eq!(
        inner.downcast_ref::<QuotaExceeded>(),
        Some(&QuotaExceeded { tenant: "a" })
    );
}

#[test]
fn membership_error_from_anyhow_comes_back_whole() {
    let original = anyhow::Error::new(QuotaExceeded { tenant: "a" }).context("check topology");
    let error = MembershipError::from(original).into_anyhow();
    assert_eq!(error.to_string(), "check topology");
    assert_eq!(
        error.downcast_ref::<QuotaExceeded>(),
        Some(&QuotaExceeded { tenant: "a" })
    );
}

#[test]
fn prefix_unavailable_keeps_its_text() {
    let error = StateMachineError::PrefixUnavailable {
        index: 4,
        applied: 7,
    };
    assert_eq!(
        error.to_string(),
        "state machine cannot snapshot Raft prefix 4; current applied index is 7"
    );
}
