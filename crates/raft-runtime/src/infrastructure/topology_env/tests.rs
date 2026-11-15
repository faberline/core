use super::*;
use crate::NodeId;
use crate::{ClusterDims, ClusterTopology};
use std::sync::Mutex;

// The standard env vars are process-global; serialize the env tests.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn replica_mode_defaults_to_single_node() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("REPLICAS_PER_SHARD");
    assert!(!replica_mode());
    std::env::set_var("REPLICAS_PER_SHARD", "1");
    assert!(!replica_mode());
    std::env::set_var("REPLICAS_PER_SHARD", "3");
    assert!(replica_mode());
    std::env::remove_var("REPLICAS_PER_SHARD");
}

#[test]
fn topology_from_env_with_local_override() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SHARD_COUNT", "1");
    std::env::set_var("REPLICAS_PER_SHARD", "3");
    std::env::set_var("VOTER_COUNT", "3");
    std::env::set_var("POD_NAME", "svc-1");
    std::env::set_var("SVC_PEERS", "10.0.0.0:9001,10.0.0.1:9002,10.0.0.2:9003");
    let t = ClusterTopology::from_env("svc", "svc-headless", 7000, "SVC_PEERS").unwrap();
    assert_eq!(t.node_id, NodeId::new(1));
    assert_eq!(
        t.membership.voters(),
        vec![NodeId::new(0), NodeId::new(1), NodeId::new(2)]
    );
    // self (id 1) excluded; peers point at the override addresses.
    assert_eq!(
        t.peers.get(&NodeId::new(0)).unwrap(),
        "http://10.0.0.0:9001"
    );
    assert_eq!(
        t.peers.get(&NodeId::new(2)).unwrap(),
        "http://10.0.0.2:9003"
    );
    assert!(!t.peers.contains_key(&NodeId::new(1)));
    let tls =
        ClusterTopology::from_env_with_scheme("svc", "svc-headless", 7000, "SVC_PEERS", "https")
            .unwrap();
    assert_eq!(
        tls.peers.get(&NodeId::new(0)).unwrap(),
        "https://10.0.0.0:9001"
    );
    assert!(
        ClusterTopology::from_env_with_scheme("svc", "svc-headless", 7000, "SVC_PEERS", "ftp",)
            .is_err()
    );
    for k in [
        "SHARD_COUNT",
        "REPLICAS_PER_SHARD",
        "VOTER_COUNT",
        "POD_NAME",
        "SVC_PEERS",
    ] {
        std::env::remove_var(k);
    }
}

#[test]
fn peer_dns_prefix_follows_the_pod_not_the_callers_binary_name() {
    // The regression this exists for: an operator names the StatefulSet
    // after the custom resource, so a CR named `quorum` produces pods
    // `quorum-0`/`quorum-1` while the binary passed its own name, `lumen`.
    // The resulting peer URL `lumen-1.<headless>` is NXDOMAIN, so no
    // RequestVote is ever delivered and every voter campaigns forever.
    // Asserted on the URL rather than on a bool, because the failure was
    // never an error — it was a well-formed address for a host that does
    // not exist.
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SHARD_COUNT", "1");
    std::env::set_var("REPLICAS_PER_SHARD", "2");
    std::env::set_var("VOTER_COUNT", "2");
    std::env::set_var("POD_NAME", "quorum-0");
    std::env::remove_var("LUMEN_PEERS");

    let t = ClusterTopology::from_env("lumen", "quorum-headless", 7373, "LUMEN_PEERS").unwrap();
    assert_eq!(t.node_id, NodeId::new(0));
    assert_eq!(
        t.peers.get(&NodeId::new(1)).unwrap(),
        "http://quorum-1.quorum-headless:7373",
        "peer URL must name the pod's own StatefulSet, not the caller's binary"
    );

    // The pre-existing deployments this must not disturb: when the CR *is*
    // named after the binary, the derived prefix equals the passed one and
    // the URL is byte-identical to what it always was.
    std::env::set_var("POD_NAME", "lumen-0");
    let same = ClusterTopology::from_env("lumen", "lumen-headless", 7373, "LUMEN_PEERS").unwrap();
    assert_eq!(
        same.peers.get(&NodeId::new(1)).unwrap(),
        "http://lumen-1.lumen-headless:7373"
    );

    for k in [
        "SHARD_COUNT",
        "REPLICAS_PER_SHARD",
        "VOTER_COUNT",
        "POD_NAME",
    ] {
        std::env::remove_var(k);
    }
}

#[test]
fn parse_peer_overrides_splits_trims_and_filters_empty() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("TEST_PEERS");
    assert!(parse_peer_overrides("TEST_PEERS").is_empty());

    std::env::set_var("TEST_PEERS", " a:1, b:2 ,,c:3");
    assert_eq!(
        parse_peer_overrides("TEST_PEERS"),
        vec!["a:1".to_string(), "b:2".to_string(), "c:3".to_string()]
    );
    std::env::remove_var("TEST_PEERS");
}

/// Clears `SHARD_ID` when dropped, so a failing test cannot leak it.
struct ShardIdGuard;

impl Drop for ShardIdGuard {
    fn drop(&mut self) {
        std::env::remove_var("SHARD_ID");
    }
}

fn set_quartet(shard_count: &str, replicas: &str, voters: &str, pod: &str) {
    std::env::set_var("SHARD_COUNT", shard_count);
    std::env::set_var("REPLICAS_PER_SHARD", replicas);
    std::env::set_var("VOTER_COUNT", voters);
    std::env::set_var("POD_NAME", pod);
}

#[test]
fn shard_id_env_is_read_into_the_topology() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _s = ShardIdGuard;
    set_quartet("3", "3", "3", "svc-s1-0");
    std::env::set_var("SHARD_ID", "1");
    std::env::remove_var("SVC_PEERS");
    assert_eq!(ClusterDims::from_env().unwrap().shard_id(), Some(1));
    let t = ClusterTopology::from_env("svc", "svc-s1-hl", 7373, "SVC_PEERS").unwrap();
    assert_eq!(t.shard_index, 1);
    assert_eq!(
        t.peers.get(&NodeId::new(1)).unwrap(),
        "http://svc-s1-1.svc-s1-hl:7373"
    );
    assert_eq!(
        t.peers.get(&NodeId::new(2)).unwrap(),
        "http://svc-s1-2.svc-s1-hl:7373"
    );
}

#[test]
fn shard_id_env_that_is_not_a_u32_names_shard_id() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _s = ShardIdGuard;
    set_quartet("3", "3", "3", "svc-s1-0");
    std::env::set_var("SHARD_ID", "abc");
    let err = ClusterDims::from_env().unwrap_err();
    assert!(
        format!("{err:#}").contains("SHARD_ID"),
        "error was: {err:#}"
    );
}

#[test]
fn shard_id_env_equal_to_shard_count_is_an_error() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _s = ShardIdGuard;
    set_quartet("3", "3", "3", "svc-s2-0");
    std::env::set_var("SHARD_ID", "3");
    std::env::remove_var("SVC_PEERS");
    let err = ClusterTopology::from_env("svc", "svc-s2-hl", 7373, "SVC_PEERS").unwrap_err();
    assert!(
        format!("{err:#}").contains("SHARD_ID"),
        "error was: {err:#}"
    );
}

#[test]
fn shard_id_env_unset_leaves_dims_without_a_shard_id() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _s = ShardIdGuard;
    set_quartet("3", "3", "3", "svc-7");
    std::env::remove_var("SHARD_ID");
    assert_eq!(ClusterDims::from_env().unwrap().shard_id(), None);
}
