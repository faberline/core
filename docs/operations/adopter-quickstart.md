# Adopter quickstart

For a new small or medium service that takes core crates for HTTP, auth,
Raft, backup and the operator. This page gives the wiring and the order; the
behavior promised by each crate is in its own `README.md`, linked below.

## Pin one tag

Depend on core by git tag. Use one tag for every core crate in the workspace
(the same rule as [`operations/README.md`](README.md)); the tag is the
workspace version `v0.5.0`:

```toml
[dependencies]
service-http = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
server-http = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
service-auth = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
raft-runtime = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
service-backup = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
service-k8s = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
```

Two tags mean two copies of every shared type, and they do not unify.

If the service depends on `index-text`, it also carries the vendored `jieba-rs`
patch in its own root manifest, because Cargo applies `[patch]` only from the
root workspace (copy `vendor/jieba-rs` next to it):

```toml
[patch.crates-io]
jieba-rs = { path = "vendor/jieba-rs" }
```

## Step 1: an empty crate with HTTP

Start a crate and add the two HTTP dependency lines:

```sh
cargo new my-service
cd my-service
```

```toml
[dependencies]
service-http = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
server-http = { git = "https://github.com/faberline/core", tag = "v0.5.0" }
```

Use `service-http` for the probe routes, tracing and the serve loop. It runs
`server-http`'s `serve_h2c_with_lifecycle` underneath, the HTTP runtime that
serves cleartext h2c and HTTP/1.1 on one listener. The crate's
[README](../../crates/service-http/README.md) has the full contract, and
[`server-http`'s README](../../crates/server-http/README.md) has the listener
contract.

- `standard_probe_routes`: the probe and documentation routes.
- `trace_layer`: request tracing.
- `serve` and `shutdown_with_drain`: serve until a signal, then drain readiness
  before stopping. `serve` passes to `serve_h2c_with_lifecycle`.

## Step 2: auth

Use `service-auth` for the request-auth layer. Attach `auth_middleware` to the
data routes with `from_fn_with_state`, passing an `Arc` of the verifier. For a
config-driven token registry, use `StaticRoleMapVerifier`; its environment
shape is `<SVC>_AUTH=off|required` and `<SVC>_TOKEN_REGISTRY_FILE`. A service
whose callers are all Kubernetes workloads can delegate to `TokenReview` and
`SubjectAccessReview` instead; see the
[README](../../crates/service-auth/README.md).

## Step 3: Raft

Use `raft-runtime` to replicate your state. Implement `RaftStateMachine`
(`apply`, `snapshot`, `restore` and `applied_index` are required; the rest have
defaults). Then:

- `RaftStore::open(dir, node_id, FsyncPolicy::Os)` opens the durable store.
- `RaftHost::spawn(node_id, membership, peers, store, state_machine, HostConfig::default())`
  starts the host.
- `RaftHost::router()` serves the peer routes; merge it into the app.
- `RaftHost::propose(command)` appends a command and returns its `Index` once
  it is applied.

The wiring below puts steps 1 to 3 together for one node. Its code is compiled
by CI: `cargo test -p service-http --examples` builds it, and clippy runs over
it with `--all-targets`.

```rust
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
```

## Step 4: backup

Use `service-backup` for scheduled backups. Its types are `BackupPolicy` and
`ScheduledBackupPolicy` (the `http-client` feature adds the HTTP client that
uses `service-auth`). Read the [README](../../crates/service-backup/README.md)
for the contract before you set a destination.

## Step 5: three replicas on Kubernetes

Use `service-k8s` to run the service as an operator-managed fleet. Implement
`ManagedService` for the service and call `service_k8s::run::<Service>()`. The
replica count is fixed at three: set `REPLICAS_PER_SHARD=3` and
`VOTER_COUNT=3`, as the replica-host tests do, and build the topology with
`ClusterTopology::from_env_with_scheme`. Read the
[README](../../crates/service-k8s/README.md) for the reconcile loop and the
resources it renders.

The three-replica host replaces the single-node `RaftStore::open` and
`RaftHost::spawn` of the wiring. Build it with `ReplicaHostBuilder`, which reads
the topology from the environment through `ClusterTopology::from_env_with_scheme`.
The topology needs `SHARD_COUNT`, `REPLICAS_PER_SHARD=3`, `VOTER_COUNT=3` and
`POD_NAME` (the pod's StatefulSet name ending in `-<ordinal>`). Call
`build_secure(data_dir, state_machine, peer_tls_env_prefix, fsync_policy, host_config)`:
it starts peers over required mutual TLS and never falls back to cleartext, and
it needs an `https` peer URL scheme. It returns a `ReplicaHostRuntime`; use `runtime.host`, an
`Arc<RaftHost>`, wherever the wiring uses the host. Read
[`crates/raft-runtime/src/app/replica_host.rs`](../../crates/raft-runtime/src/app/replica_host.rs)
for the exact order of startup.

## Compatibility

Which core tag a service may pin, and what a breaking change needs, is the
[compatibility and pinning policy](README.md#compatibility-and-pinning).

## Runbook

Releasing core and running a downstream pin are in
[`operations/README.md`](README.md) and
[`release-pipeline.md`](release-pipeline.md).
