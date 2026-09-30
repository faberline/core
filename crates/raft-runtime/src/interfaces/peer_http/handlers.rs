use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use raft_core::{AppendResp, InstallSnapshotReq, InstallSnapshotResp, NodeId, RaftMsg, VoteResp};

use super::wire::{
    AppendEnvelope, CapableSnapEnvelope, CapableSnapshotResp, SnapEnvelope, TimeoutNowEnvelope,
    VoteEnvelope,
};
use crate::application::{take_reply, Shared, StorageFailed};

pub(crate) async fn request_vote(
    State(s): State<Arc<Shared>>,
    Json(env): Json<VoteEnvelope>,
) -> axum::response::Response {
    if env.group_id != s.group_id.0 {
        return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
    }
    let mut n = s.node.lock().await;
    n.handle(env.from, RaftMsg::Vote(env.req));
    if s.persist(&n).is_err() {
        return Json(VoteResp {
            term: 0,
            granted: false,
        })
        .into_response();
    }
    Json(match take_reply(&mut n, env.from) {
        Some(RaftMsg::VoteResp(r)) => r,
        _ => VoteResp {
            term: 0,
            granted: false,
        },
    })
    .into_response()
}

pub(crate) async fn append_entries(
    State(s): State<Arc<Shared>>,
    Json(env): Json<AppendEnvelope>,
) -> axum::response::Response {
    if env.group_id != s.group_id.0 {
        return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
    }
    let mut n = s.node.lock().await;
    n.handle(env.from, RaftMsg::Append(env.req));
    if s.persist(&n).is_err() {
        return Json(AppendResp {
            term: 0,
            success: false,
            match_index: 0,
        })
        .into_response();
    }
    s.apply_ready(&mut n);
    Json(match take_reply(&mut n, env.from) {
        Some(RaftMsg::AppendResp(r)) => r,
        _ => AppendResp {
            term: 0,
            success: false,
            match_index: 0,
        },
    })
    .into_response()
}

pub(crate) async fn install_snapshot(
    State(s): State<Arc<Shared>>,
    Json(env): Json<SnapEnvelope>,
) -> axum::response::Response {
    if env.group_id != s.group_id.0 {
        return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
    }
    Json(install_snapshot_response(&s, env.from, env.req).await).into_response()
}

pub(crate) async fn install_snapshot_capable(
    State(s): State<Arc<Shared>>,
    Json(env): Json<CapableSnapEnvelope>,
) -> axum::response::Response {
    if env.group_id != s.group_id.0 {
        return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
    }
    if env.snapshot_nonce == 0
        || s.sm.snapshot_capability() != Some(env.snapshot_capability.as_str())
    {
        return (
            StatusCode::CONFLICT,
            "snapshot capability is not supported by this voter",
        )
            .into_response();
    }
    let response = install_snapshot_response(&s, env.from, env.req).await;
    Json(CapableSnapshotResp {
        term: response.term,
        accepted: response.accepted,
        snapshot_index: response.snapshot_index,
        snapshot_capability: env.snapshot_capability,
        snapshot_nonce: env.snapshot_nonce,
    })
    .into_response()
}

async fn install_snapshot_response(
    s: &Arc<Shared>,
    from: NodeId,
    req: InstallSnapshotReq,
) -> InstallSnapshotResp {
    let serial = Arc::clone(&s.snapshot_install).lock_owned().await;
    let shared = Arc::clone(s);
    match tokio::task::spawn_blocking(move || {
        let _serial = serial;
        shared.install_snapshot_serial(from, req)
    })
    .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::error!(%error, "raft: snapshot install worker failed");
            *s.latched_failure.lock().unwrap() = Some(StorageFailed {
                node_id: s.id,
                operation: "state-machine-restore-panic",
                path: s.store_path.clone(),
                kind: std::io::ErrorKind::InvalidData,
            });
            InstallSnapshotResp {
                term: 0,
                accepted: false,
                snapshot_index: 0,
            }
        }
    }
}

pub(crate) async fn timeout_now(
    State(s): State<Arc<Shared>>,
    Json(env): Json<TimeoutNowEnvelope>,
) -> axum::response::Response {
    if env.group_id != s.group_id.0 {
        return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
    }
    let mut n = s.node.lock().await;
    n.handle(env.from, RaftMsg::TimeoutNow(env.req));
    if s.persist(&n).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    StatusCode::OK.into_response()
}
