use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use super::{
    add_learner_handler, append_entries, demote_handler, install_snapshot,
    install_snapshot_capable, members_handler, promote_handler, publish_handler, raftz,
    read_index_handler, remove_handler, request_vote, timeout_now, transfer_handler,
};
use crate::application::RaftHost;

impl RaftHost {
    /// Peer raft RPCs + producer forward + status; merge into the service app so
    /// they ride the h2c serve port.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/raft/request-vote", post(request_vote))
            .route("/raft/append-entries", post(append_entries))
            .route("/raft/install-snapshot", post(install_snapshot))
            .route(
                "/raft/install-snapshot-capable",
                post(install_snapshot_capable),
            )
            .route("/raft/timeout-now", post(timeout_now))
            .route(Self::PUBLISH_PATH, post(publish_handler))
            .route(Self::READ_INDEX_PATH, post(read_index_handler))
            .route("/raftz", get(raftz))
            .route("/raft/admin/add-learner", post(add_learner_handler))
            .route("/raft/admin/promote", post(promote_handler))
            .route("/raft/admin/demote", post(demote_handler))
            .route("/raft/admin/remove", post(remove_handler))
            .route("/raft/admin/transfer-leadership", post(transfer_handler))
            .route("/raft/admin/members", get(members_handler))
            .with_state(Arc::clone(&self.shared))
    }
}
