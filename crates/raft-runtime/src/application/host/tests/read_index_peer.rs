use super::*;
use std::io::{Read, Write};

use crate::domain::ReadIndexReply;
use crate::{RaftStore, StateMachineError};

/// A state machine that only records the applied index.
struct ReadIndexSm {
    applied: AtomicU64,
}

impl RaftStateMachine for ReadIndexSm {
    fn apply(&self, index: Index, _command: &[u8]) -> Result<(), StateMachineError> {
        self.applied.store(index.get(), Ordering::SeqCst);
        Ok(())
    }
    fn snapshot(&self, writer: &mut dyn Write) -> Result<(), StateMachineError> {
        writer.write_all(b"{}").map_err(StateMachineError::other)?;
        Ok(())
    }
    fn restore(&self, reader: &mut dyn Read) -> Result<(), StateMachineError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(StateMachineError::other)?;
        Ok(())
    }
    fn applied_index(&self) -> Index {
        Index::new(self.applied.load(Ordering::SeqCst))
    }
}

struct ReadIndexTestHost {
    host: RaftHost,
    _directory: tempfile::TempDir,
}

impl std::ops::Deref for ReadIndexTestHost {
    type Target = RaftHost;
    fn deref(&self) -> &Self::Target {
        &self.host
    }
}

/// A single-voter host that has elected itself leader.
async fn elected_single_host() -> ReadIndexTestHost {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    let sm = Arc::new(ReadIndexSm {
        applied: AtomicU64::new(0),
    });
    let host = RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0)], vec![]),
        HashMap::new(),
        RaftStore::open(
            path.to_str().unwrap(),
            NodeId::new(0),
            crate::FsyncPolicy::Os,
        )
        .unwrap(),
        sm as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    );
    {
        let mut node = host.shared.node.lock().await;
        for _ in 0..raft_core::ELECTION_TIMEOUT_FLOOR_TICKS {
            node.tick();
        }
    }
    ReadIndexTestHost {
        host,
        _directory: dir,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_peer_client_read_index_grants_from_a_leader() {
    let leader = elected_single_host().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, leader.router()).await.unwrap();
    });
    let client = elected_single_host().await;
    let reply = client
        .shared
        .peer_client
        .read_index(&format!("http://{address}"))
        .await;
    server.abort();
    assert!(
        matches!(reply, ReadIndexReply::Granted { .. }),
        "a single-voter leader must grant the read index, got {reply:?}"
    );
}

#[tokio::test]
async fn http_peer_client_read_index_fails_when_nothing_listens() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let client = elected_single_host().await;
    let reply = client
        .shared
        .peer_client
        .read_index(&format!("http://{address}"))
        .await;
    assert!(
        matches!(reply, ReadIndexReply::Failed { .. }),
        "a closed port must map to Failed, got {reply:?}"
    );
}
