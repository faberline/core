use std::sync::Arc;

use axum::extract::State;
use axum::Json;

pub(crate) use crate::application::host_status;
use crate::application::{RaftStatus, Shared};

pub(crate) async fn raftz(State(s): State<Arc<Shared>>) -> Json<RaftStatus> {
    Json(host_status(&s).await)
}
