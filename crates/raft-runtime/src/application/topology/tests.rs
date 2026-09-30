use super::*;

fn dims(shard_count: u32, replicas_per_shard: u32, voter_count: u32, pod: &str) -> ClusterDims {
    ClusterDims {
        shard_count,
        replicas_per_shard,
        voter_count,
        pod_name: pod.into(),
    }
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
