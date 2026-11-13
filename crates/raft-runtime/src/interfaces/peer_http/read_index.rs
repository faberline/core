use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use super::wire::{NotLeader, ReadIndexEnvelope, ReadIndexResp};
use crate::application::{ReadIndexError, Shared};

/// Leader-side linearizable read index (#187 part 2): the leader's read index
/// after a quorum round; `421` with a leader hint if this node is not the
/// leader.
pub(crate) async fn read_index_handler(
    State(s): State<Arc<Shared>>,
    Json(env): Json<ReadIndexEnvelope>,
) -> axum::response::Response {
    if env.group_id != s.group_id.as_str() {
        return (StatusCode::BAD_REQUEST, "group id mismatch").into_response();
    }
    match s.read_index_leader().await {
        Ok(index) => (StatusCode::OK, Json(ReadIndexResp { index: index.get() })).into_response(),
        Err(ReadIndexError::NotLeader) => {
            let leader = {
                let n = s.node.lock().await;
                s.leader_url(&n).0
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
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
