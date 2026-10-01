use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use super::{
    append_entries, install_snapshot, install_snapshot_capable, publish_handler, raftz,
    request_vote, timeout_now,
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
            .route("/raftz", get(raftz))
            .with_state(Arc::clone(&self.shared))
    }
}
