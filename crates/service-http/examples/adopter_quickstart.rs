//! Adopter quickstart: one HTTP service with auth on its data route and a
//! single-node Raft host behind it. CI compiles this file; the quickstart doc
//! copies it byte for byte.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use raft_runtime::{
    FsyncPolicy, HostConfig, Index, Membership, NodeId, RaftHost, RaftStateMachine, RaftStore,
    StateMachineError,
};
use server_lifecycle::DrainController;
use service_auth::{auth_middleware, StaticRoleMapVerifier};
use service_http::{serve, shutdown_with_drain, standard_probe_routes, trace_layer};

#[derive(utoipa::OpenApi)]
#[openapi(info(title = "adopter_quickstart", description = "adopter quickstart"))]
struct ApiDoc;

fn openapi() -> utoipa::openapi::OpenApi {
    use utoipa::OpenApi as _;
    ApiDoc::openapi()
}

/// The domain state: it remembers the last applied index and nothing else.
struct Counter {
    applied: AtomicU64,
}

impl RaftStateMachine for Counter {
    fn apply(&self, index: Index, _command: &[u8]) -> Result<(), StateMachineError> {
        self.applied.store(index.get(), Ordering::Release);
        Ok(())
    }

    fn snapshot(&self, _writer: &mut dyn Write) -> Result<(), StateMachineError> {
        Ok(())
    }

    fn restore(&self, _reader: &mut dyn Read) -> Result<(), StateMachineError> {
        Ok(())
    }

    fn applied_index(&self) -> Index {
        Index::new(self.applied.load(Ordering::Acquire))
    }
}

async fn propose(
    State(host): State<Arc<RaftHost>>,
    body: Bytes,
) -> Result<String, (StatusCode, String)> {
    let index = host
        .propose(body.to_vec())
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    Ok(index.get().to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let readiness = Arc::new(DrainController::new());

    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let store = RaftStore::open(&data_dir, NodeId::new(0), FsyncPolicy::Os)?;
    let host = Arc::new(RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0)], vec![]),
        HashMap::new(),
        store,
        Arc::new(Counter {
            applied: AtomicU64::new(0),
        }),
        HostConfig::default(),
    ));

    // Auth is off here (`open`). Use `StaticRoleMapVerifier::new(true, tokens)` to require tokens.
    let verifier = Arc::new(StaticRoleMapVerifier::open());
    let data = Router::new()
        .route("/commands", post(propose))
        .with_state(host.clone())
        .layer(axum::middleware::from_fn_with_state(
            verifier,
            auth_middleware::<StaticRoleMapVerifier>,
        ));

    let app = standard_probe_routes(readiness.clone(), None, openapi)
        .merge(data)
        .merge(host.router())
        .layer(trace_layer());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let readiness_drain = readiness.clone();
    serve(
        listener,
        app,
        shutdown_with_drain(
            move || readiness_drain.start_drain(),
            Duration::from_secs(2),
        ),
    )
    .await;
    Ok(())
}
