//! Integration tests for `RaftHost::status()` and its equivalence to `/raftz`.

use std::time::{Duration, Instant};

use raft_runtime::{NodeId, RaftStatus};

use crate::support::cluster;
use cluster::{await_leader, cluster};

fn h2c_client() -> reqwest::Client {
    reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap()
}

async fn raftz_status(client: &reqwest::Client, url: &str) -> RaftStatus {
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
        let s = raftz_status(client, url).await;
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

fn assert_stable_fields_equal(a: &RaftStatus, b: &RaftStatus) {
    assert_eq!(a.group_id, b.group_id);
    assert_eq!(a.id, b.id);
    assert_eq!(a.role, b.role);
    assert_eq!(a.is_leader, b.is_leader);
    assert_eq!(a.leader, b.leader);
    assert_eq!(a.committed_voters, b.committed_voters);
    assert_eq!(a.incoming_voters, b.incoming_voters);
    assert_eq!(a.learners, b.learners);
    assert_eq!(a.membership_phase, b.membership_phase);
}

#[tokio::test]
async fn leader_status() {
    let nodes = cluster(3).await;
    let leader_idx = await_leader(&nodes)
        .await
        .expect("a three-voter cluster elects a leader");
    let leader = &nodes[leader_idx];
    let own_id = NodeId::new(leader_idx as u64);

    let status = leader.host.status().await;
    assert_eq!(status.role, "Leader");
    assert!(status.is_leader);
    assert_eq!(status.leader, Some(own_id));

    let client = h2c_client();
    let http_status = raftz_status(&client, &leader.url).await;
    assert_stable_fields_equal(&http_status, &status);
}

#[tokio::test]
async fn follower_status() {
    let nodes = cluster(3).await;
    let leader_idx = await_leader(&nodes)
        .await
        .expect("a three-voter cluster elects a leader");
    let leader_id = NodeId::new(leader_idx as u64);
    let follower_idx = (leader_idx + 1) % 3;
    let follower = &nodes[follower_idx];

    let status = follower.host.status().await;
    assert_eq!(status.role, "Follower");
    assert!(!status.is_leader);
    assert_eq!(status.leader, Some(leader_id));

    let client = h2c_client();
    let http_status = raftz_status(&client, &follower.url).await;
    assert_stable_fields_equal(&http_status, &status);
}

#[tokio::test]
async fn learner_status() {
    let nodes = cluster(3).await;
    let leader_idx = await_leader(&nodes)
        .await
        .expect("a three-voter cluster elects a leader");
    let target_idx = (leader_idx + 1) % 3;
    let target_id = NodeId::new(target_idx as u64);

    nodes[leader_idx]
        .host
        .demote_voter(target_id)
        .await
        .expect("demoting a voter succeeds");

    let client = h2c_client();
    let demoted_http_status = poll_status_until(
        &client,
        &nodes[target_idx].url,
        |s| s.role == "Learner" && s.learners.contains(&target_id),
        Duration::from_secs(5),
        "demoted node reports Learner role and target in learners",
    )
    .await;

    let status = nodes[target_idx].host.status().await;
    assert_eq!(status.role, "Learner");
    assert!(status.learners.contains(&target_id));

    assert_stable_fields_equal(&demoted_http_status, &status);
}
