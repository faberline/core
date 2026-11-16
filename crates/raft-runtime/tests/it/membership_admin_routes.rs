//! Membership admin routes under `/raft/admin/` on a running host (core#398).
//!
//! The controller calls these routes on the leader. Each route runs the same
//! host operation as the `RaftHost` method of the same name, and a follower
//! answers `421` with the `not-leader` body, as the publish route does.

use std::time::{Duration, Instant};

use raft_runtime::{NodeId, RaftStatus};
use serde_json::{json, Value};

use crate::support::cluster::{await_leader, cluster, Node};

fn h2c_client() -> reqwest::Client {
    reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap()
}

async fn status(client: &reqwest::Client, url: &str) -> RaftStatus {
    client
        .get(format!("{url}/raftz"))
        .send()
        .await
        .expect("a host serves its own status")
        .json()
        .await
        .expect("the status is the published shape")
}

async fn poll_status_until<F>(
    client: &reqwest::Client,
    url: &str,
    predicate: F,
    timeout: Duration,
    desc: &str,
) -> RaftStatus
where
    F: Fn(&RaftStatus) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let s = status(client, url).await;
        if predicate(&s) {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for condition: {desc} at {url}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// POST `body` to `path` on `node`, and return the status and the JSON answer.
async fn post(client: &reqwest::Client, node: &Node, path: &str, body: Value) -> (u16, Value) {
    let resp = client
        .post(format!("{}{path}", node.url))
        .json(&body)
        .send()
        .await
        .expect("the route answers");
    let code = resp.status().as_u16();
    let answer = resp.json::<Value>().await.unwrap_or(Value::Null);
    (code, answer)
}

/// GET `/raft/admin/members` on `node`.
async fn members(client: &reqwest::Client, node: &Node) -> (u16, Value) {
    let resp = client
        .get(format!("{}/raft/admin/members", node.url))
        .send()
        .await
        .expect("the route answers");
    let code = resp.status().as_u16();
    let answer = resp.json::<Value>().await.unwrap_or(Value::Null);
    (code, answer)
}

/// The member with `id` in a members answer, or `None`.
fn member(answer: &Value, id: u64) -> Option<&Value> {
    answer["members"].as_array()?.iter().find(|m| m["id"] == id)
}

async fn poll_members_until<F>(
    client: &reqwest::Client,
    node: &Node,
    predicate: F,
    timeout: Duration,
    desc: &str,
) -> Value
where
    F: Fn(&Value) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let (code, answer) = members(client, node).await;
        if code == 200 && predicate(&answer) {
            return answer;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for members condition: {desc}; last answer {code} {answer}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn promoting_a_caught_up_learner_through_the_route_makes_it_a_voter() {
    let nodes = cluster(4).await;
    let leader = await_leader(&nodes)
        .await
        .expect("a four-voter cluster elects a leader");
    let client = h2c_client();
    let target_id = (leader as u64 + 1) % 4;
    let target = NodeId::new(target_id);
    nodes[leader]
        .host
        .demote_voter(target)
        .await
        .expect("a voter of four can be demoted");
    poll_status_until(
        &client,
        &nodes[leader].url,
        |s| s.learners.contains(&target),
        Duration::from_secs(5),
        "target is a learner",
    )
    .await;

    let body = json!({ "group_id": nodes[leader].host.group_id().as_str(), "target": target_id });
    let deadline = Instant::now() + Duration::from_secs(5);
    let (code, answer) = loop {
        let (code, answer) =
            post(&client, &nodes[leader], "/raft/admin/promote", body.clone()).await;
        if code == 200 || Instant::now() >= deadline {
            break (code, answer);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(code, 200, "promote answer: {answer}");
    assert!(
        answer["index"].is_number(),
        "promote returns the index: {answer}"
    );

    poll_members_until(
        &client,
        &nodes[leader],
        |a| member(a, target_id).is_some_and(|m| m["role"] == "voter"),
        Duration::from_secs(5),
        "target is a voter",
    )
    .await;
}

#[tokio::test]
async fn promoting_a_node_that_is_not_a_member_is_refused_with_409() {
    // Negative control: without the membership guard the promote would
    // make node 99 a voter.
    let nodes = cluster(4).await;
    let leader = await_leader(&nodes)
        .await
        .expect("a four-voter cluster elects a leader");
    let client = h2c_client();
    let body = json!({ "group_id": nodes[leader].host.group_id().as_str(), "target": 99 });
    let (code, answer) = post(&client, &nodes[leader], "/raft/admin/promote", body).await;
    assert_eq!(code, 409, "promote answer: {answer}");
    assert_eq!(answer["refusal"], "NotALearner");
    assert!(
        answer["message"]
            .as_str()
            .is_some_and(|m| m.contains("not a learner")),
        "message: {answer}"
    );

    let (code, answer) = members(&client, &nodes[leader]).await;
    assert_eq!(code, 200, "members answer: {answer}");
    assert!(
        member(&answer, 99).is_none(),
        "node 99 is not a member: {answer}"
    );
    let voters = answer["members"]
        .as_array()
        .map_or(0, |ms| ms.iter().filter(|m| m["role"] == "voter").count());
    assert_eq!(voters, 4, "the voter count is unchanged: {answer}");
}

#[tokio::test]
async fn a_follower_answers_every_admin_route_with_421() {
    let nodes = cluster(4).await;
    let leader = await_leader(&nodes)
        .await
        .expect("a four-voter cluster elects a leader");
    let follower = (leader + 1) % 4;
    let client = h2c_client();
    let group_id = nodes[follower].host.group_id().as_str().to_string();
    let leader_id = status(&client, &nodes[leader].url).await.id.get();

    for (path, body) in [
        (
            "/raft/admin/add-learner",
            json!({ "group_id": group_id, "target": 42, "url": "http://127.0.0.1:9" }),
        ),
        (
            "/raft/admin/promote",
            json!({ "group_id": group_id, "target": 1 }),
        ),
        (
            "/raft/admin/demote",
            json!({ "group_id": group_id, "target": 1 }),
        ),
        (
            "/raft/admin/remove",
            json!({ "group_id": group_id, "target": 1 }),
        ),
        (
            "/raft/admin/transfer-leadership",
            json!({ "group_id": group_id, "target": 1 }),
        ),
    ] {
        let (code, answer) = post(&client, &nodes[follower], path, body).await;
        assert_eq!(code, 421, "{path} on a follower: {answer}");
        assert_eq!(answer["error"], "not-leader", "{path}: {answer}");
        assert_eq!(answer["leader"], leader_id, "{path}: {answer}");
    }

    let (code, answer) = members(&client, &nodes[follower]).await;
    assert_eq!(code, 421, "members on a follower: {answer}");
    assert_eq!(answer["error"], "not-leader", "members: {answer}");
    assert_eq!(answer["leader"], leader_id, "members: {answer}");
}

#[tokio::test]
async fn add_learner_then_remove_through_the_routes() {
    let nodes = cluster(4).await;
    let leader = await_leader(&nodes)
        .await
        .expect("a four-voter cluster elects a leader");
    let client = h2c_client();
    let group_id = nodes[leader].host.group_id().as_str().to_string();

    let (code, answer) = post(
        &client,
        &nodes[leader],
        "/raft/admin/add-learner",
        json!({ "group_id": group_id, "target": 42, "url": "http://127.0.0.1:9" }),
    )
    .await;
    assert_eq!(code, 200, "add-learner answer: {answer}");
    assert!(
        answer["index"].is_number(),
        "add-learner returns the index: {answer}"
    );

    let added = poll_members_until(
        &client,
        &nodes[leader],
        |a| member(a, 42).is_some(),
        Duration::from_secs(5),
        "node 42 is listed",
    )
    .await;
    let m42 = member(&added, 42).expect("node 42 is listed");
    assert_eq!(m42["role"], "learner", "{added}");
    assert_eq!(m42["caught_up"], false, "{added}");

    let (code, answer) = post(
        &client,
        &nodes[leader],
        "/raft/admin/remove",
        json!({ "group_id": group_id, "target": 42 }),
    )
    .await;
    assert_eq!(code, 200, "remove answer: {answer}");

    poll_members_until(
        &client,
        &nodes[leader],
        |a| member(a, 42).is_none(),
        Duration::from_secs(5),
        "node 42 is gone",
    )
    .await;
}

#[tokio::test]
async fn transfer_to_a_non_voter_is_refused_with_409() {
    let nodes = cluster(4).await;
    let leader = await_leader(&nodes)
        .await
        .expect("a four-voter cluster elects a leader");
    let client = h2c_client();
    let body = json!({ "group_id": nodes[leader].host.group_id().as_str(), "target": 99 });
    let (code, answer) = post(
        &client,
        &nodes[leader],
        "/raft/admin/transfer-leadership",
        body,
    )
    .await;
    assert_eq!(code, 409, "transfer answer: {answer}");
    assert!(!answer["refusal"].is_null(), "a typed refusal: {answer}");
    assert!(
        answer["message"].as_str().is_some_and(|m| !m.is_empty()),
        "message: {answer}"
    );
}

#[tokio::test]
async fn a_wrong_group_id_is_a_400() {
    let nodes = cluster(4).await;
    let leader = await_leader(&nodes)
        .await
        .expect("a four-voter cluster elects a leader");
    let client = h2c_client();
    let body = json!({ "group_id": "no-such-group", "target": 1 });
    let (code, answer) = post(&client, &nodes[leader], "/raft/admin/promote", body).await;
    assert_eq!(code, 400, "promote answer: {answer}");
}
