//! The peer client port: the host's calls to other members that are not raft
//! messages (status reads and proposal forwarding), plus the peer address book
//! those calls and raft message delivery share.

use std::future::Future;
use std::io;
use std::pin::Pin;

/// The leader-side write target. The direct router, registry router, and
/// follower forward client all use this one path.
pub(crate) const PUBLISH_PATH: &str = "/raft/publish";

/// What a leader answered to a forwarded proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForwardReply {
    /// The leader applied the command at log index `seq`.
    Accepted { seq: u64 },
    /// The leader's state machine refused the command before append and asked
    /// the caller to retry later.
    Backpressure {
        reason: String,
        retry_after_seconds: u64,
    },
    /// The leader refused the command before append.
    Rejected { reason: String },
    /// The request failed or the reply was not understood; the command may or
    /// may not have been appended.
    Failed { reason: String },
}

/// The peer client port, generic over the peer id so that this layer names
/// no raft-core item.
pub(crate) trait PeerClient<Id>: Send + Sync {
    /// The base URL registered for `peer`.
    fn address(&self, peer: &Id) -> Option<String>;

    /// Register or replace the base URL of `peer`.
    fn set_address(&self, peer: Id, url: String);

    /// Forget the base URL of `peer`.
    fn remove_address(&self, peer: &Id);

    /// When `peer` has no registered address, count and log the discarded
    /// in-flight message and return `true`.
    fn discard_if_withdrawn(&self, peer: &Id) -> bool;

    /// How many in-flight messages were discarded because their peer's
    /// address had been withdrawn.
    fn withdrawn_address_drops(&self) -> u64;

    /// Read the raw `/raftz` status body of `peer`.
    fn read_status(
        &self,
        peer: Id,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + Send + '_>>;

    /// Forward `command` to the leader at `leader_url`.
    fn forward<'a>(
        &'a self,
        leader_url: &'a str,
        command: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = ForwardReply> + Send + 'a>>;
}
