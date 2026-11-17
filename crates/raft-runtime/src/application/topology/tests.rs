use super::*;

fn dims(shard_count: u32, replicas_per_shard: u32, voter_count: u32, pod: &str) -> ClusterDims {
    ClusterDims::new(shard_count, replicas_per_shard, voter_count, pod)
}

#[test]
fn pod_prefix_is_the_statefulset_name_and_rejects_a_nameless_pod() {
    // Hyphenated CR names are ordinary (`my-search-cluster-3`), so the split
    // has to be the LAST hyphen, matching `pod_ordinal`'s.
    assert_eq!(dims(1, 1, 1, "quorum-0").pod_prefix().unwrap(), "quorum");
    assert_eq!(
        dims(1, 1, 1, "my-search-cluster-3").pod_prefix().unwrap(),
        "my-search-cluster"
    );
    // No prefix to derive: refuse rather than emit `http://-1.headless`.
    assert!(dims(1, 1, 1, "-0").pod_prefix().is_err());
    assert!(dims(1, 1, 1, "nohyphen").pod_prefix().is_err());
}

#[test]
fn cluster_dims_derives_shard_and_replica_from_pod_ordinal() {
    // 3 shards × 3 replicas: pod-7 → shard 1, replica 2.
    let d = dims(3, 3, 3, "svc-7");
    assert_eq!(d.pod_ordinal().unwrap(), 7);
    assert_eq!(d.shard_index().unwrap(), 1);
    assert_eq!(d.replica_index().unwrap(), 2);
    assert!(d.is_voter().unwrap());

    let d = dims(3, 3, 2, "svc-8");
    assert_eq!(d.shard_index().unwrap(), 2);
    assert_eq!(d.replica_index().unwrap(), 2);
    assert!(
        !d.is_voter().unwrap(),
        "replica 2 is a learner when voter_count=2"
    );
}

#[test]
fn cluster_dims_pod_ordinal_rejects_bad_suffix() {
    assert!(dims(3, 3, 3, "svc-").pod_ordinal().is_err());
    assert!(dims(3, 3, 3, "svc-abc").pod_ordinal().is_err());
    assert!(dims(3, 3, 3, "svc").pod_ordinal().is_err());
}

#[test]
fn peer_ordinal_matches_replica_times_shard_count_plus_shard() {
    assert_eq!(peer_ordinal(3, 1, 2), 7); // 2*3+1
    assert_eq!(peer_ordinal(1, 0, 4), 4);
}

#[test]
fn shard_id_makes_replica_index_the_pod_ordinal() {
    let d = dims(3, 3, 3, "svc-s2-1").with_shard_id(2);
    assert_eq!(d.shard_id(), Some(2));
    assert_eq!(d.shard_index().unwrap(), 2);
    assert_eq!(d.replica_index().unwrap(), 1);

    let d = dims(3, 3, 3, "svc-7");
    assert_eq!(d.shard_id(), None);
    assert_eq!(d.shard_index().unwrap(), 1);
    assert_eq!(d.replica_index().unwrap(), 2);
}

#[test]
fn shard_id_topology_uses_the_replica_as_the_peer_ordinal() {
    let d = dims(3, 3, 3, "svc-s1-0").with_shard_id(1);
    let t = ClusterTopology::from_dims(&d, "svc", "svc-s1-hl", 7373, &[], "http").unwrap();
    assert_eq!(t.node_id, NodeId::new(0));
    assert_eq!(t.shard_index, 1);
    assert_eq!(
        t.peers.get(&NodeId::new(1)).unwrap(),
        "http://svc-s1-1.svc-s1-hl:7373"
    );
    assert_eq!(
        t.peers.get(&NodeId::new(2)).unwrap(),
        "http://svc-s1-2.svc-s1-hl:7373"
    );
    assert_eq!(t.peers.len(), 2);
}

#[test]
fn shard_id_not_below_shard_count_is_an_error() {
    let d = dims(3, 3, 3, "svc-s2-0").with_shard_id(3);
    let err = ClusterTopology::from_dims(&d, "svc", "svc-s2-hl", 7373, &[], "http").unwrap_err();
    assert!(
        format!("{err:#}").contains("SHARD_ID"),
        "error was: {err:#}"
    );
}

#[test]
fn without_shard_id_peers_use_the_old_ordinal_math() {
    let d = dims(3, 3, 3, "svc-1");
    let t = ClusterTopology::from_dims(&d, "svc", "svc-hl", 7373, &[], "http").unwrap();
    assert_eq!(t.node_id, NodeId::new(0));
    assert_eq!(
        t.peers.get(&NodeId::new(1)).unwrap(),
        "http://svc-4.svc-hl:7373"
    );
    assert_eq!(
        t.peers.get(&NodeId::new(2)).unwrap(),
        "http://svc-7.svc-hl:7373"
    );
}

#[test]
fn a_shard_pod_beyond_the_initial_replicas_boots_as_a_joiner() {
    let d = dims(3, 3, 3, "svc-s1-3").with_shard_id(1);
    let t = ClusterTopology::from_dims(&d, "svc", "svc-s1-hl", 7373, &[], "http").unwrap();
    assert!(t.joiner);
    assert_eq!(t.node_id, NodeId::new(3));
    assert!(t.membership.voters().is_empty());
    assert!(t.membership.learners().is_empty());
    assert_eq!(t.peers.len(), 3);
    for replica in 0..3u64 {
        assert_eq!(
            t.peers.get(&NodeId::new(replica)).unwrap(),
            &format!("http://svc-s1-{replica}.svc-s1-hl:7373")
        );
    }
}

#[test]
fn a_shard_pod_inside_the_initial_replicas_is_not_a_joiner() {
    let d = dims(3, 3, 3, "svc-s1-2").with_shard_id(1);
    let t = ClusterTopology::from_dims(&d, "svc", "svc-s1-hl", 7373, &[], "http").unwrap();
    assert!(!t.joiner);
    assert_eq!(t.node_id, NodeId::new(2));
    assert_eq!(t.membership.voters().len(), 3);
}

#[test]
fn without_shard_id_an_ordinal_outside_the_replicas_is_still_an_error() {
    let d = dims(3, 3, 3, "svc-9");
    let err = ClusterTopology::from_dims(&d, "svc", "svc-hl", 7373, &[], "http").unwrap_err();
    assert!(
        format!("{err:#}").contains("outside REPLICAS_PER_SHARD"),
        "error was: {err:#}"
    );
}
