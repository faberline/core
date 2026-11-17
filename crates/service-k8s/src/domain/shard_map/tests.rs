use super::*;

fn report(shard: u32, buckets: &[u32], map_version: u64) -> PodShardReport {
    PodShardReport {
        shard,
        buckets: buckets.to_vec(),
        map_version,
    }
}

#[test]
fn balanced_is_round_robin() {
    assert_eq!(
        ShardMap::balanced(8, 2),
        ShardMap {
            version: 1,
            buckets: vec![0, 1, 0, 1, 0, 1, 0, 1],
        }
    );
    assert_eq!(ShardMap::balanced(4, 0).buckets, vec![0, 0, 0, 0]);
    assert!(ShardMap::balanced(0, 3).buckets.is_empty());
}

#[test]
fn with_moved_buckets_raises_the_version_and_changes_only_the_named_buckets() {
    let original = ShardMap::balanced(8, 2);

    let moved = original.with_moved_buckets(&[0, 2], 1);

    assert_eq!(moved.version, 2);
    assert_eq!(moved.buckets, vec![1, 1, 1, 1, 0, 1, 0, 1]);
    assert_eq!(original.version, 1);

    let outside = original.with_moved_buckets(&[99], 1);
    assert_eq!(outside.version, 2);
    assert_eq!(outside.buckets, original.buckets);
}

#[test]
fn the_configmap_round_trips() {
    let m = ShardMap::balanced(8, 3).with_moved_buckets(&[1], 2);

    let cm = m.to_configmap("svc-shard-map", "acme");

    assert_eq!(cm["kind"], "ConfigMap");
    assert_eq!(cm["metadata"]["name"], "svc-shard-map");
    assert_eq!(cm["metadata"]["namespace"], "acme");
    assert!(cm["data"][SHARD_MAP_KEY].as_str().is_some());
    assert_eq!(ShardMap::from_configmap(&cm), Ok(m));
}

#[test]
fn from_configmap_rejects_a_missing_key_and_bad_json() {
    assert!(matches!(
        ShardMap::from_configmap(&json!({"data": {}})),
        Err(ShardMapError::InvalidConfigMap(_))
    ));
    assert!(matches!(
        ShardMap::from_configmap(&json!({"data": {"shard-map.json": "not json"}})),
        Err(ShardMapError::InvalidConfigMap(_))
    ));
    assert!(matches!(
        ShardMap::from_configmap(&json!({})),
        Err(ShardMapError::InvalidConfigMap(_))
    ));
}

#[test]
fn rebuild_from_three_complete_reports_returns_the_newest_version() {
    let reports = [
        report(0, &[0, 3], 5),
        report(1, &[1, 4], 5),
        report(2, &[2, 5], 5),
    ];

    assert_eq!(
        rebuild_shard_map(&reports, 6),
        Ok(ShardMap {
            version: 5,
            buckets: vec![0, 1, 2, 0, 1, 2],
        })
    );
}

#[test]
fn a_stale_report_is_ignored() {
    let reports = [
        report(0, &[0, 1, 2, 3], 4),
        report(1, &[0, 2], 7),
        report(2, &[1, 3], 7),
    ];

    assert_eq!(
        rebuild_shard_map(&reports, 4),
        Ok(ShardMap {
            version: 7,
            buckets: vec![1, 2, 1, 2],
        })
    );
}

#[test]
fn reports_that_disagree_at_the_same_version_conflict() {
    let reports = [report(0, &[0, 1], 3), report(1, &[1], 3)];

    assert_eq!(
        rebuild_shard_map(&reports, 2),
        Err(ShardMapError::Conflict {
            version: 3,
            bucket: 1,
        })
    );
}

#[test]
fn a_bucket_that_no_report_owns_is_missing() {
    let partial = [report(0, &[0, 1], 2), report(1, &[3], 2)];
    assert_eq!(
        rebuild_shard_map(&partial, 4),
        Err(ShardMapError::MissingBucket(2))
    );

    // Five buckets, but the reports cover only 0..=3: the last one is not
    // silently dropped.
    let short = [
        report(0, &[0, 3], 1),
        report(1, &[1], 1),
        report(2, &[2], 1),
    ];
    assert_eq!(
        rebuild_shard_map(&short, 5),
        Err(ShardMapError::MissingBucket(4))
    );
}

#[test]
fn a_bucket_outside_the_count_and_no_reports_are_errors() {
    assert_eq!(rebuild_shard_map(&[], 4), Err(ShardMapError::NoReports));
    assert_eq!(
        rebuild_shard_map(&[report(0, &[0, 9], 1)], 4),
        Err(ShardMapError::BucketOutOfRange {
            bucket: 9,
            bucket_count: 4,
        })
    );
}

#[test]
fn the_same_shard_may_report_a_bucket_twice() {
    let reports = [report(0, &[0, 1], 1), report(0, &[0], 1)];

    assert_eq!(
        rebuild_shard_map(&reports, 2),
        Ok(ShardMap {
            version: 1,
            buckets: vec![0, 0],
        })
    );
}
