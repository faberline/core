//! The inbound half of the peer transport: the Vote / Append / InstallSnapshot /
//! TimeoutNow handlers, the leader-side publish target, `/raftz` status, the
//! single-host router and the multi-group registry's router. The membership
//! admin routes live under `/raft/admin/`.

mod admin;
mod handlers;
mod publish;
mod read_index;
mod registry;
mod router;
mod status;
pub(crate) mod wire;

pub(crate) use status::host_status;

use admin::{
    add_learner_handler, demote_handler, members_handler, promote_handler, remove_handler,
    transfer_handler,
};
use handlers::{
    append_entries, install_snapshot, install_snapshot_capable, request_vote, timeout_now,
};
use publish::publish_handler;
use read_index::read_index_handler;
use status::raftz;
