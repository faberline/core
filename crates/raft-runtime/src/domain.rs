//! Domain layer: the raft-runtime values and ports that both the application
//! and the infrastructure layer use. Nothing here names a raft-core item.

mod group;

pub use group::{GroupId, LEGACY_GROUP_ID};
