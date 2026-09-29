use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::extract::{FromRequest, Request, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::application::{decode_backpressure, ProposalOutcome, Shared};
use crate::infrastructure::{NotLeader, PublishEnvelope};

const PUBLISH_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// Leader-side write target (the redirect destination): propose + apply, return
/// the seq; or `421` with a leader hint if this node is not the leader.
pub(crate) async fn publish_handler(
    State(s): State<Arc<Shared>>,
    request: Request,
) -> axum::response::Response {
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, PUBLISH_BODY_LIMIT).await {
        Ok(body) => body,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };

    // A well-formed foreign envelope is a caller error on every host. This is
    // the only body verdict that precedes the follower routing response.
    if let Ok(env) = serde_json::from_slice::<PublishEnvelope>(&body) {
        if env.group_id != s.group_id.0 {
            return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
        }
    }

    // Read both values in one node-lock snapshot. A follower must answer with
    // its route before media-type or JSON validation, but must not return a
    // leader hint that came from a different node state.
    let leader = {
        let n = s.node.lock().await;
        if n.is_leader() {
            None
        } else {
            Some(s.leader_url(&n).0)
        }
    };
    if let Some(leader) = leader {
        return (
            StatusCode::MISDIRECTED_REQUEST,
            Json(NotLeader {
                error: "not-leader",
                leader,
            }),
        )
            .into_response();
    }

    // Reconstruct the original request so the elected leader retains Axum's
    // standard Json extractor behavior: 415 for media type, 400 for syntax,
    // and 422 for an invalid envelope shape.
    let request = Request::from_parts(parts, Body::from(body));
    let Json(env) = match Json::<PublishEnvelope>::from_request(request, &()).await {
        Ok(env) => env,
        Err(rejection) => return rejection.into_response(),
    };
    if env.group_id != s.group_id.0 {
        return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
    }
    match s.try_propose_applied(env.command).await {
        Some(ProposalOutcome::Completed { index: seq }) => {
            (StatusCode::OK, Json(serde_json::json!({ "seq": seq }))).into_response()
        }
        None => {
            let leader = {
                let node = s.node.lock().await;
                s.leader_url(&node).0
            };
            (
                StatusCode::MISDIRECTED_REQUEST,
                Json(NotLeader {
                    error: "not-leader",
                    leader,
                }),
            )
                .into_response()
        }
        Some(ProposalOutcome::RejectedBeforeAdmission { reason }) => {
            if let Some(backpressure) = decode_backpressure(&reason) {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(
                        axum::http::header::RETRY_AFTER,
                        backpressure.retry_after_seconds.to_string(),
                    )],
                    Json(serde_json::json!({
                        "outcome": "rejected_before_admission",
                        "error": backpressure.reason,
                        "retry_after_seconds": backpressure.retry_after_seconds,
                    })),
                )
                    .into_response()
            } else {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "outcome": "rejected_before_admission",
                        "error": reason,
                    })),
                )
                    .into_response()
            }
        }
        Some(ProposalOutcome::Ambiguous { reason, .. }) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": reason })),
        )
            .into_response(),
        Some(ProposalOutcome::DurabilityFailure { failure, .. }) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": failure.to_string() })),
        )
            .into_response(),
    }
}
