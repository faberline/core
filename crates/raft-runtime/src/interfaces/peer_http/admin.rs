//! Membership admin routes under `/raft/admin/` (core#398).
//!
//! Route contract:
//! - Every route: a `group_id` that is not this group's answers `400 group id mismatch`.
//!   A follower answers `421` with `{"error":"not-leader","leader":<id>}`, as the
//!   publish route does.
//! - `POST add-learner` `{group_id,target,url}`: registers the address, then admits the
//!   learner. `200 {"index"}`, or `409 {"refusal","message"}`.
//! - `POST promote` `{group_id,target}`: `409` with refusal `NotALearner` unless `target`
//!   is a learner now (raft-core would make a non-member a voter). Else `200 {"index"}`
//!   or `409`.
//! - `POST demote`, `POST remove`: `200 {"index"}` or `409`.
//! - `POST transfer-leadership` `{group_id,target}`: `200 {"transferring_to"}` or `409`.
//! - `GET members`: `{group_id,leader,membership_phase,members}`, the members sorted by
//!   `id`, each with its `role` (`voter` or `learner`) and `caught_up`.

use std::fmt::Debug;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use raft_core::{Index, NodeId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::wire::NotLeader;
use crate::application::{host_status, AdmissionRefused, MembershipPhase, RaftStatus, Shared};

#[derive(Deserialize)]
pub(crate) struct AddLearnerBody {
    group_id: String,
    target: NodeId,
    url: String,
}

#[derive(Deserialize)]
pub(crate) struct TargetBody {
    group_id: String,
    target: NodeId,
}

#[derive(Serialize)]
struct MemberView {
    id: NodeId,
    role: &'static str,
    caught_up: bool,
}

#[derive(Serialize)]
struct MembersView {
    group_id: String,
    leader: Option<NodeId>,
    membership_phase: MembershipPhase,
    members: Vec<MemberView>,
}

/// `400` when the body names another group.
fn group_mismatch(s: &Shared, group_id: &str) -> Option<Response> {
    (s.group_id.as_str() != group_id)
        .then(|| (StatusCode::BAD_REQUEST, "group id mismatch").into_response())
}

/// `421` with the leader hint when this node is not the leader.
async fn not_leader(s: &Shared) -> Option<Response> {
    let n = s.node.lock().await;
    if n.is_leader() {
        return None;
    }
    let leader = s.leader_url(&n).0;
    Some(
        (
            StatusCode::MISDIRECTED_REQUEST,
            Json(NotLeader {
                error: "not-leader",
                leader,
            }),
        )
            .into_response(),
    )
}

fn index_ok(idx: Index) -> Response {
    (StatusCode::OK, Json(json!({ "index": idx.get() }))).into_response()
}

fn refusal_response(refusal: Value, message: String) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({ "refusal": refusal, "message": message })),
    )
        .into_response()
}

/// `409` with the Debug text as the message; the refusal is its serde value.
fn refused<R: Serialize + Debug>(r: R) -> Response {
    let message = format!("{r:?}");
    let refusal = serde_json::to_value(&r).unwrap_or(Value::Null);
    refusal_response(refusal, message)
}

/// `409` for a learner refusal; the message is its `Display` text.
fn admission_refused(r: AdmissionRefused) -> Response {
    let refusal = serde_json::to_value(r).unwrap_or(Value::Null);
    refusal_response(refusal, r.to_string())
}

/// True when the member `id` has the leader's commit index (the leader itself counts).
fn caught_up(status: &RaftStatus, id: NodeId) -> bool {
    id == status.id
        || status
            .peer_match_index
            .iter()
            .any(|p| p.id == id && p.match_index >= status.commit_index)
}

pub(crate) async fn add_learner_handler(
    State(s): State<Arc<Shared>>,
    Json(body): Json<AddLearnerBody>,
) -> Response {
    if let Some(r) = group_mismatch(&s, &body.group_id) {
        return r;
    }
    if let Some(r) = not_leader(&s).await {
        return r;
    }
    s.do_upsert_peer(body.target, body.url).await;
    match s.do_add_learner(body.target).await {
        Ok(idx) => index_ok(idx),
        Err(r) => admission_refused(r),
    }
}

pub(crate) async fn promote_handler(
    State(s): State<Arc<Shared>>,
    Json(body): Json<TargetBody>,
) -> Response {
    if let Some(r) = group_mismatch(&s, &body.group_id) {
        return r;
    }
    if let Some(r) = not_leader(&s).await {
        return r;
    }
    if !host_status(&s).await.learners.contains(&body.target) {
        return refusal_response(
            json!("NotALearner"),
            format!("node {} is not a learner of this group", body.target.get()),
        );
    }
    match s.do_promote_learner(body.target).await {
        Ok(idx) => index_ok(idx),
        Err(r) => refused(r),
    }
}

pub(crate) async fn demote_handler(
    State(s): State<Arc<Shared>>,
    Json(body): Json<TargetBody>,
) -> Response {
    if let Some(r) = group_mismatch(&s, &body.group_id) {
        return r;
    }
    if let Some(r) = not_leader(&s).await {
        return r;
    }
    match s.do_demote_voter(body.target).await {
        Ok(idx) => index_ok(idx),
        Err(r) => refused(r),
    }
}

pub(crate) async fn remove_handler(
    State(s): State<Arc<Shared>>,
    Json(body): Json<TargetBody>,
) -> Response {
    if let Some(r) = group_mismatch(&s, &body.group_id) {
        return r;
    }
    if let Some(r) = not_leader(&s).await {
        return r;
    }
    match s.do_remove_member(body.target).await {
        Ok(idx) => index_ok(idx),
        Err(r) => refused(r),
    }
}

pub(crate) async fn transfer_handler(
    State(s): State<Arc<Shared>>,
    Json(body): Json<TargetBody>,
) -> Response {
    if let Some(r) = group_mismatch(&s, &body.group_id) {
        return r;
    }
    if let Some(r) = not_leader(&s).await {
        return r;
    }
    match s.do_transfer_leadership(body.target).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "transferring_to": body.target.get() })),
        )
            .into_response(),
        Err(r) => refused(r),
    }
}

pub(crate) async fn members_handler(State(s): State<Arc<Shared>>) -> Response {
    if let Some(r) = not_leader(&s).await {
        return r;
    }
    let status = host_status(&s).await;
    let voters = status.committed_voters.iter().map(|&id| MemberView {
        id,
        role: "voter",
        caught_up: caught_up(&status, id),
    });
    let learners = status.learners.iter().map(|&id| MemberView {
        id,
        role: "learner",
        caught_up: caught_up(&status, id),
    });
    let mut members: Vec<MemberView> = voters.chain(learners).collect();
    members.sort_by_key(|m| m.id);
    (
        StatusCode::OK,
        Json(MembersView {
            group_id: status.group_id.clone(),
            leader: status.leader,
            membership_phase: status.membership_phase,
            members,
        }),
    )
        .into_response()
}
