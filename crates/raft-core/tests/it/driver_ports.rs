//! The driver ports: a node survives a restart through `RaftStorage`, and a
//! driver elects a leader by sending every outgoing message through
//! `RaftDelivery`.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::{pin, Pin};
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use raft_core::{
    auto_membership, CommandLease, Index, InstallSnapshotReq, InstallSnapshotResp, NodeId,
    PersistedState, PersistedStateRef, PinnedCommand, RaftDelivery, RaftMsg, RaftNode, RaftStorage,
    Term,
};

#[derive(Default)]
struct MemStorage {
    state: Mutex<Option<PersistedState>>,
}

struct MemPin(Vec<u8>);
struct MemLease(Vec<u8>);

impl PinnedCommand for MemPin {
    fn map(self: Box<Self>) -> io::Result<Box<dyn CommandLease>> {
        Ok(Box::new(MemLease(self.0)))
    }
}

impl CommandLease for MemLease {
    fn command(&self) -> &[u8] {
        &self.0
    }
}

impl RaftStorage for MemStorage {
    fn load(&self) -> io::Result<Option<PersistedState>> {
        Ok(self.state.lock().unwrap().clone())
    }

    fn save(&self, state: &PersistedStateRef<'_>) -> io::Result<()> {
        *self.state.lock().unwrap() = Some(PersistedState {
            term: state.term,
            voted_for: state.voted_for,
            log: state.log.to_vec(),
            commit_index: state.commit_index,
            snapshot_index: state.snapshot_index,
            snapshot_term: state.snapshot_term,
            snapshot: state.snapshot.to_vec(),
            conf: state.conf.cloned(),
        });
        Ok(())
    }

    fn pin_committed_command(
        &self,
        index: Index,
        term: Term,
    ) -> io::Result<Box<dyn PinnedCommand>> {
        let state = self.state.lock().unwrap();
        let entry = state
            .as_ref()
            .and_then(|s| s.log.iter().find(|e| e.index == index && e.term == term))
            .ok_or_else(|| io::Error::other("no such committed entry"))?;
        Ok(Box::new(MemPin(entry.command.clone())))
    }
}

/// Delivers from one fixed sender to in-memory peers, answering with the
/// first message the peer addresses back.
struct Loopback {
    from: NodeId,
    peers: Mutex<HashMap<NodeId, RaftNode>>,
}

impl RaftDelivery for Loopback {
    fn deliver(
        &self,
        to: NodeId,
        msg: RaftMsg,
    ) -> Pin<Box<dyn Future<Output = Option<RaftMsg>> + Send + '_>> {
        Box::pin(async move {
            let mut peers = self.peers.lock().unwrap();
            let peer = peers.get_mut(&to)?;
            peer.handle(self.from, msg);
            peer.take_outgoing()
                .into_iter()
                .find(|o| o.to == self.from)
                .map(|o| o.msg)
        })
    }

    fn install_snapshot(
        &self,
        _to: NodeId,
        _req: InstallSnapshotReq,
        _required_capability: Option<&'static str>,
    ) -> Pin<Box<dyn Future<Output = io::Result<InstallSnapshotResp>> + Send + '_>> {
        Box::pin(async { Err(io::Error::other("no snapshots in this test")) })
    }
}

fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("in-memory port future was pending"),
    }
}

#[test]
fn a_node_restarts_from_what_it_saved_through_the_storage_port() {
    let membership = auto_membership(1);
    let storage: Box<dyn RaftStorage> = Box::new(MemStorage::default());
    let mut node = RaftNode::new(0, &membership);
    for _ in 0..100 {
        if node.is_leader() {
            break;
        }
        node.tick();
    }
    assert!(node.is_leader());
    let index = node.propose(b"put k v".to_vec()).expect("leader accepts");
    storage.save(&node.persisted_ref()).unwrap();
    let committed = node.take_committed();
    let entry = committed.iter().find(|e| e.index == index).unwrap();

    let restored = RaftNode::from_persisted(0, &membership, storage.load().unwrap().unwrap());
    assert_eq!(restored.persisted(), node.persisted());
    let lease = storage
        .pin_committed_command(index, entry.term)
        .unwrap()
        .map()
        .unwrap();
    assert_eq!(lease.command(), b"put k v");
}

#[test]
fn a_driver_elects_a_leader_through_the_delivery_port() {
    let membership = auto_membership(3);
    let delivery = Loopback {
        from: 0,
        peers: Mutex::new(
            (1..3)
                .map(|id| (id, RaftNode::new(id, &membership)))
                .collect(),
        ),
    };
    let delivery: &dyn RaftDelivery = &delivery;
    let mut node = RaftNode::new(0, &membership);
    for _ in 0..100 {
        if node.is_leader() {
            break;
        }
        node.tick();
        for out in node.take_outgoing() {
            if let Some(reply) = ready(delivery.deliver(out.to, out.msg)) {
                node.handle(out.to, reply);
            }
        }
    }
    assert!(node.is_leader());
}
