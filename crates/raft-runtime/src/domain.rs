//! Domain layer: the raft-runtime values and ports that both the application
//! and the infrastructure layer use. Nothing here names a raft-core item.

mod group;
mod peer_client;

pub use group::{GroupId, LEGACY_GROUP_ID};
pub(crate) use peer_client::{
    ForwardReply, PeerClient, ReadIndexReply, PUBLISH_PATH, READ_INDEX_PATH,
};
