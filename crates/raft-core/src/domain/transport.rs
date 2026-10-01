use super::ids::NodeId;
use super::message::Outgoing;

/// How a driver delivers a node's outgoing messages. The production driver
/// implements this over h2c; tests use an in-process bus.
pub trait RaftTransport {
    fn deliver(&mut self, from: NodeId, out: Outgoing);
}
