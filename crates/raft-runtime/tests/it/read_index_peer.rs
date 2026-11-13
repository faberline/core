//! Peer read-index route (#187 part 2): the leader answers with its read index,
//! a follower answers `421` with a leader hint, and the router and registry
//! route the path by group.

use std::sync::Arc;
use std::time::Duration;

use raft_runtime::{HostConfig, RaftHost, RaftRegistry};

use crate::support::cluster;
use cluster::{await_leader_with_tick, bind, cluster_with_config};

fn read_cfg() -> HostConfig {
    HostConfig::default().with_propose_timeout(Duration::from_millis(600))
}

fn h2c_client() -> reqwest::Client {
    reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .expect("the h2c client builds")
}

fn read_index_url(base_url: &str) -> String {
    format!("{base_url}{}", RaftHost::READ_INDEX_PATH)
}

#[tokio::test]
async fn leader_read_index_endpoint_returns_the_index() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");
    nodes[leader]
        .host
        .propose(vec![1])
        .await
        .expect("first proposal commits");
    nodes[leader]
        .host
        .propose(vec![2])
        .await
        .expect("second proposal commits");
    let group_id = nodes[leader].host.group_id().as_str().to_owned();

    let response = h2c_client()
        .post(read_index_url(&nodes[leader].url))
        .json(&serde_json::json!({ "group_id": group_id }))
        .send()
        .await
        .expect("the read-index request reaches the leader");
    let status = response.status();
    let body = response
        .text()
        .await
        .expect("the response body is readable");
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "the leader must answer 200; body: {body}"
    );
    let json: serde_json::Value = serde_json::from_str(&body).expect("the leader answers JSON");
    let index = json["index"].as_u64().expect("index is a u64");
    assert!(index >= 2, "read index {index} is below 2");
}

#[tokio::test]
async fn follower_read_index_endpoint_answers_421_with_a_leader_hint() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");
    let follower = (0..nodes.len())
        .find(|&i| i != leader)
        .expect("a follower exists");
    let group_id = nodes[follower].host.group_id().as_str().to_owned();

    let response = h2c_client()
        .post(read_index_url(&nodes[follower].url))
        .json(&serde_json::json!({ "group_id": group_id }))
        .send()
        .await
        .expect("the read-index request reaches the follower");
    let status = response.status();
    let body = response
        .text()
        .await
        .expect("the response body is readable");
    assert_eq!(
        status,
        reqwest::StatusCode::MISDIRECTED_REQUEST,
        "a follower must answer 421; body: {body}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&body).expect("the follower response is JSON");
    assert_eq!(
        json["error"], "not-leader",
        "a follower must name the not-leader error; body: {body}"
    );
    assert_eq!(
        json["leader"],
        serde_json::json!(leader as u64),
        "a follower must hint the leader; body: {body}"
    );
}

#[tokio::test]
async fn read_index_endpoint_rejects_a_foreign_group() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");

    let response = h2c_client()
        .post(read_index_url(&nodes[leader].url))
        .json(&serde_json::json!({ "group_id": "other" }))
        .send()
        .await
        .expect("the foreign-group request reaches the leader");
    let status = response.status();
    let body = response
        .text()
        .await
        .expect("the response body is readable");
    assert_eq!(
        status,
        reqwest::StatusCode::BAD_REQUEST,
        "the leader must reject a foreign group as 400; body: {body}"
    );
}

#[tokio::test]
async fn registry_read_index_endpoint_routes_by_group_and_404s_unknown() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");
    // The read-index route answers 503 until the leader commits in its own term (#300).
    nodes[leader]
        .host
        .propose(vec![1])
        .await
        .expect("first proposal commits, so the leader has committed in its own term");
    let group_id = nodes[leader].host.group_id().as_str().to_owned();

    let registry = RaftRegistry::new();
    registry
        .register(Arc::clone(&nodes[leader].host))
        .expect("a fresh registry accepts its one host");
    let (listener, url) = bind().await;
    let router = registry.router();
    let serve = tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let router = router.clone();
                tokio::spawn(async move {
                    let _ = transport_h2c::server::serve_connection(stream, router).await;
                });
            }
        }
    });
    let client = h2c_client();

    let known = client
        .post(read_index_url(&url))
        .json(&serde_json::json!({ "group_id": group_id }))
        .send()
        .await
        .expect("the known-group request reaches the registry");
    let known_status = known.status();
    let known_body = known.text().await.expect("the response body is readable");
    assert_eq!(
        known_status,
        reqwest::StatusCode::OK,
        "the registry must route a known leader group to 200; body: {known_body}"
    );

    let unknown = client
        .post(read_index_url(&url))
        .json(&serde_json::json!({ "group_id": "no-such-group" }))
        .send()
        .await
        .expect("the unknown-group request reaches the registry");
    assert_eq!(
        unknown.status(),
        reqwest::StatusCode::NOT_FOUND,
        "the registry must answer 404 for an unknown group"
    );

    serve.abort();
}
