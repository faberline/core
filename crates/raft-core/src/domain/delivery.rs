//! The message-delivery port a driver sends a node's outgoing messages
//! through.

use std::future::Future;
use std::io;
use std::pin::Pin;

use super::ids::NodeId;
use super::message::{InstallSnapshotReq, InstallSnapshotResp, RaftMsg};

/// Delivers one [`RaftMsg`] to a peer and returns the peer's reply.
///
/// A driver drains [`RaftNode::take_outgoing`](super::RaftNode::take_outgoing)
/// and hands each message to this port. It feeds a returned reply back with
/// [`RaftNode::handle`](super::RaftNode::handle).
pub trait RaftDelivery: Send + Sync {
    /// Send `msg` to `to`. `None` means the peer did not answer or the
    /// message has no reply; the node retries through its own timers.
    /// An `InstallSnapshot` sent here requires no capability.
    fn deliver(
        &self,
        to: NodeId,
        msg: RaftMsg,
    ) -> Pin<Box<dyn Future<Output = Option<RaftMsg>> + Send + '_>>;

    /// Install a snapshot on `to`. With `required_capability`, the peer must
    /// echo that driver-defined token, or the install is an error.
    fn install_snapshot(
        &self,
        to: NodeId,
        req: InstallSnapshotReq,
        required_capability: Option<&'static str>,
    ) -> Pin<Box<dyn Future<Output = io::Result<InstallSnapshotResp>> + Send + '_>>;
}
