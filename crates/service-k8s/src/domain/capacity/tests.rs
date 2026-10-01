use super::*;

fn policy(min: u32, max: u32) -> ReplicaLayerPolicy {
    ReplicaLayerPolicy {
        min_replicas_per_shard: min,
        max_replicas_per_shard: max,
        target_cpu_utilization: 70,
        target_memory_utilization: 80,
    }
}

#[test]
fn cpu_scale_out_is_a_whole_shard_layer() {
    let plan = plan_replica_layer(
        3,
        2,
        policy(2, 5),
        ObservedUtilization {
            cpu_percent: Some(90),
            memory_percent: Some(60),
        },
    )
    .unwrap();
    assert_eq!(plan.desired_replicas_per_shard, 3);
    assert_eq!(plan.desired_total_pods, 9);
    assert!(plan.requires_membership_change());
}

#[test]
fn memory_can_drive_the_larger_layer() {
    let plan = plan_replica_layer(
        2,
        2,
        policy(1, 6),
        ObservedUtilization {
            cpu_percent: Some(40),
            memory_percent: Some(170),
        },
    )
    .unwrap();
    assert_eq!(plan.desired_replicas_per_shard, 5);
    assert_eq!(plan.desired_total_pods, 10);
}

#[test]
fn disk_split_threshold_is_strictly_greater_than_one_gib() {
    let policy = ShardSplitPolicy::default();
    let at_threshold = plan_shard_split(
        1,
        policy,
        &[ObservedShardUsage {
            shard_index: 0,
            durable_bytes: DEFAULT_SHARD_SPLIT_THRESHOLD_BYTES,
        }],
    )
    .unwrap();
    assert!(!at_threshold.requires_split());

    let crossed = plan_shard_split(
        1,
        policy,
        &[ObservedShardUsage {
            shard_index: 0,
            durable_bytes: DEFAULT_SHARD_SPLIT_THRESHOLD_BYTES + 1,
        }],
    )
    .unwrap();
    assert!(crossed.requires_split());
    assert_eq!(crossed.desired_shard_count, 2);
}

#[test]
fn disk_split_adds_one_shard_and_honors_the_ceiling() {
    let policy = ShardSplitPolicy {
        split_threshold_bytes: 100,
        max_shards: Some(4),
    };
    let usage = [
        ObservedShardUsage {
            shard_index: 0,
            durable_bytes: 101,
        },
        ObservedShardUsage {
            shard_index: 1,
            durable_bytes: 500,
        },
        ObservedShardUsage {
            shard_index: 2,
            durable_bytes: 500,
        },
    ];
    let split = plan_shard_split(3, policy, &usage).unwrap();
    assert_eq!(split.desired_shard_count, 4);
    assert_eq!(split.busiest_shard.unwrap().shard_index, 1);

    let at_limit = plan_shard_split(
        4,
        policy,
        &[ObservedShardUsage {
            shard_index: 1,
            durable_bytes: 500,
        }],
    )
    .unwrap();
    assert!(!at_limit.requires_split());
    assert!(at_limit.max_shards_reached);
}

#[test]
fn disk_split_rejects_invalid_policy_and_observations() {
    assert_eq!(
        plan_shard_split(0, ShardSplitPolicy::default(), &[]),
        Err(ShardSplitError::ZeroShards)
    );
    assert_eq!(
        plan_shard_split(
            1,
            ShardSplitPolicy {
                split_threshold_bytes: 0,
                max_shards: None,
            },
            &[]
        ),
        Err(ShardSplitError::ZeroThreshold)
    );
    assert_eq!(
        plan_shard_split(
            2,
            ShardSplitPolicy {
                split_threshold_bytes: 1,
                max_shards: Some(1),
            },
            &[]
        ),
        Err(ShardSplitError::InvalidMaximum)
    );
    assert_eq!(
        plan_shard_split(
            1,
            ShardSplitPolicy::default(),
            &[ObservedShardUsage {
                shard_index: 1,
                durable_bytes: 1,
            }]
        ),
        Err(ShardSplitError::UnknownShard)
    );
}

#[test]
fn missing_metrics_hold_and_bounds_clamp() {
    let held = plan_replica_layer(4, 3, policy(2, 5), ObservedUtilization::default()).unwrap();
    assert_eq!(held.desired_replicas_per_shard, 3);
    assert_eq!(held.desired_total_pods, 12);

    let floor = plan_replica_layer(
        4,
        3,
        policy(2, 5),
        ObservedUtilization {
            cpu_percent: Some(1),
            memory_percent: Some(1),
        },
    )
    .unwrap();
    assert_eq!(floor.desired_replicas_per_shard, 2);
    assert_eq!(floor.desired_total_pods, 8);
}

#[test]
fn invalid_partial_topologies_are_rejected() {
    assert_eq!(
        plan_replica_layer(0, 1, policy(1, 3), ObservedUtilization::default()),
        Err(ReplicaLayerError::ZeroShards)
    );
    assert_eq!(
        plan_replica_layer(1, 1, policy(3, 2), ObservedUtilization::default()),
        Err(ReplicaLayerError::InvalidBounds)
    );
}
