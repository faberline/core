//! The multi-group registry's router: exposes multiple independently durable
//! consensus groups behind a single `/raft/*` HTTP/2 listener, routing each
//! incoming RPC to its designated group by `group_id`.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::wire::{
    AppendEnvelope, PublishEnvelope, SnapEnvelope, TimeoutNowEnvelope, VoteEnvelope,
};
use super::{
    append_entries, host_status, install_snapshot, publish_handler, request_vote, timeout_now,
};
use crate::application::{RaftHost, RaftRegistry, RaftStatus};

impl RaftRegistry {
    /// Build the unified `/raft/*` router for all registered groups.
    ///
    /// Routes incoming RPCs to the matching group by `group_id`.
    /// An unknown or unregistered group returns `404 Not Found`.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/raft/request-vote", post(request_vote_demux))
            .route("/raft/append-entries", post(append_entries_demux))
            .route("/raft/install-snapshot", post(install_snapshot_demux))
            .route("/raft/timeout-now", post(timeout_now_demux))
            .route(RaftHost::PUBLISH_PATH, post(publish_demux))
            .route("/raftz", get(raftz_demux))
            .with_state(self.clone())
    }
}

async fn request_vote_demux(
    State(reg): State<RaftRegistry>,
    Json(env): Json<VoteEnvelope>,
) -> axum::response::Response {
    let host = reg.host(&env.group_id);
    match host {
        Some(h) => request_vote(State(Arc::clone(&h.shared)), Json(env)).await,
        None => (StatusCode::NOT_FOUND, "group not found").into_response(),
    }
}

async fn append_entries_demux(
    State(reg): State<RaftRegistry>,
    Json(env): Json<AppendEnvelope>,
) -> axum::response::Response {
    let host = reg.host(&env.group_id);
    match host {
        Some(h) => append_entries(State(Arc::clone(&h.shared)), Json(env)).await,
        None => (StatusCode::NOT_FOUND, "group not found").into_response(),
    }
}

async fn install_snapshot_demux(
    State(reg): State<RaftRegistry>,
    Json(env): Json<SnapEnvelope>,
) -> axum::response::Response {
    let host = reg.host(&env.group_id);
    match host {
        Some(h) => install_snapshot(State(Arc::clone(&h.shared)), Json(env)).await,
        None => (StatusCode::NOT_FOUND, "group not found").into_response(),
    }
}

async fn timeout_now_demux(
    State(reg): State<RaftRegistry>,
    Json(env): Json<TimeoutNowEnvelope>,
) -> axum::response::Response {
    let host = reg.host(&env.group_id);
    match host {
        Some(h) => timeout_now(State(Arc::clone(&h.shared)), Json(env)).await,
        None => (StatusCode::NOT_FOUND, "group not found").into_response(),
    }
}

async fn publish_demux(
    State(reg): State<RaftRegistry>,
    request: Request,
) -> axum::response::Response {
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, 2 * 1024 * 1024).await {
        Ok(body) => body,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };
    let envelope = serde_json::from_slice::<PublishEnvelope>(&body);
    let host = match &envelope {
        Ok(env) => reg.host(&env.group_id),
        Err(_) => reg.sole_host(),
    };
    match host {
        Some(h) => {
            let request = Request::from_parts(parts, Body::from(body));
            publish_handler(State(Arc::clone(&h.shared)), request).await
        }
        None => {
            if envelope.is_ok() {
                (StatusCode::NOT_FOUND, "group not found").into_response()
            } else {
                (StatusCode::BAD_REQUEST, "invalid publish envelope").into_response()
            }
        }
    }
}

async fn raftz_demux(State(reg): State<RaftRegistry>) -> Json<BTreeMap<String, RaftStatus>> {
    let mut map = BTreeMap::new();
    for h in reg.hosts() {
        let s = host_status(&h.shared).await;
        map.insert(h.shared.group_id.as_str().to_owned(), s);
    }
    Json(map)
}
