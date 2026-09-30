use super::*;
use crate::ClusterTopology;
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
    assert_eq!(t.node_id, 1);
    assert_eq!(t.membership.voters(), vec![0, 1, 2]);
    // self (id 1) excluded; peers point at the override addresses.
    assert_eq!(t.peers.get(&0).unwrap(), "http://10.0.0.0:9001");
    assert_eq!(t.peers.get(&2).unwrap(), "http://10.0.0.2:9003");
    assert!(!t.peers.contains_key(&1));
    let tls =
        ClusterTopology::from_env_with_scheme("svc", "svc-headless", 7000, "SVC_PEERS", "https")
            .unwrap();
    assert_eq!(tls.peers.get(&0).unwrap(), "https://10.0.0.0:9001");
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
    assert_eq!(t.node_id, 0);
    assert_eq!(
        t.peers.get(&1).unwrap(),
        "http://quorum-1.quorum-headless:7373",
        "peer URL must name the pod's own StatefulSet, not the caller's binary"
    );

    // The pre-existing deployments this must not disturb: when the CR *is*
    // named after the binary, the derived prefix equals the passed one and
    // the URL is byte-identical to what it always was.
    std::env::set_var("POD_NAME", "lumen-0");
    let same = ClusterTopology::from_env("lumen", "lumen-headless", 7373, "LUMEN_PEERS").unwrap();
    assert_eq!(
        same.peers.get(&1).unwrap(),
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
