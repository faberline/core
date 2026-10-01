//! The inbound half of the peer transport: the Vote / Append / InstallSnapshot /
//! TimeoutNow handlers, the leader-side publish target, `/raftz` status, the
//! single-host router and the multi-group registry's router.

mod handlers;
mod publish;
mod registry;
mod router;
mod status;
pub(crate) mod wire;

pub(crate) use status::host_status;

use handlers::{
    append_entries, install_snapshot, install_snapshot_capable, request_vote, timeout_now,
};
use publish::publish_handler;
use status::raftz;
