//! A joiner outside the initial membership is not ready and never campaigns
//! until the leader adds it (#394).

use std::time::{Duration, Instant};

use raft_runtime::{HostConfig, NodeId};

use crate::member_admission::{h2c_client, poll_status_until, spawn_standalone_node, status};
use crate::support::cluster::{await_leader, cluster, leader_wait_budget};

#[tokio::test]
async fn a_joiner_outside_the_membership_stays_not_ready_and_never_campaigns_until_added() {
    let nodes = cluster(1).await;
    assert_eq!(await_leader(&nodes).await, Some(0));

    let joiner = spawn_standalone_node(1).await;
    let id = NodeId::new(1);
    assert!(!joiner.host.is_ready().await);

    // One election window passes: a campaign would raise the term.
    let window = leader_wait_budget(HostConfig::default().tick(), 2).wait_budget;
    tokio::time::sleep(window).await;
    let client = h2c_client();
    let s = status(&client, &joiner.url).await;
    assert_eq!(s.role, "Learner");
    assert_eq!(s.term, 0);
    assert_eq!(s.leader, None);
    assert!(!joiner.host.is_ready().await);

    nodes[0].host.upsert_peer(id, joiner.url.clone()).await;
    nodes[0]
        .host
        .add_learner(id)
        .await
        .expect("admitting a routable learner succeeds");

    let deadline = Instant::now() + Duration::from_secs(5);
    while !joiner.host.is_ready().await {
        assert!(Instant::now() < deadline, "joiner never became ready");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let s = poll_status_until(
        &client,
        &joiner.url,
        |s| s.learners.contains(&id),
        Duration::from_secs(5),
        "joiner reports itself as a learner",
    )
    .await;
    assert_eq!(s.leader, Some(NodeId::new(0)));
}

#[tokio::test]
async fn a_voter_in_the_initial_membership_still_elects_and_becomes_ready() {
    let nodes = cluster(1).await;
    assert_eq!(await_leader(&nodes).await, Some(0));

    let deadline = Instant::now() + Duration::from_secs(5);
    while !nodes[0].host.is_ready().await {
        assert!(Instant::now() < deadline, "voter never became ready");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(nodes[0].host.is_leader().await);
}
