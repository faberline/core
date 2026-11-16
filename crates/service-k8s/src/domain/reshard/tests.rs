use super::*;

fn usage(shard_index: u32, durable_bytes: u64) -> ObservedShardUsage {
    ObservedShardUsage {
        shard_index,
        durable_bytes,
    }
}

fn policy(threshold: u64, max_shards: Option<u32>) -> ShardSplitPolicy {
    ShardSplitPolicy {
        split_threshold_bytes: threshold,
        max_shards,
    }
}

#[test]
fn exactly_the_threshold_holds_and_one_byte_more_splits() {
    assert_eq!(
        decide_split(2, policy(100, None), &[usage(0, 100), usage(1, 10)]),
        SplitDecision::Hold
    );
    assert_eq!(
        decide_split(2, policy(100, None), &[usage(0, 101), usage(1, 10)]),
        SplitDecision::Split {
            from_shard: 0,
            new_shard_count: 3
        }
    );
}

#[test]
fn the_busiest_shard_splits_and_a_tie_keeps_the_lowest_index() {
    assert_eq!(
        decide_split(
            3,
            policy(100, None),
            &[usage(0, 150), usage(1, 400), usage(2, 200)]
        ),
        SplitDecision::Split {
            from_shard: 1,
            new_shard_count: 4
        }
    );
    assert_eq!(
        decide_split(
            3,
            policy(100, None),
            &[usage(0, 300), usage(1, 300), usage(2, 10)]
        ),
        SplitDecision::Split {
            from_shard: 0,
            new_shard_count: 4
        }
    );
}

#[test]
fn reaching_max_shards_holds() {
    assert_eq!(
        decide_split(3, policy(100, Some(3)), &[usage(0, 10_000)]),
        SplitDecision::Hold
    );
    assert_eq!(
        decide_split(3, policy(100, Some(4)), &[usage(0, 10_000)]),
        SplitDecision::Split {
            from_shard: 0,
            new_shard_count: 4
        }
    );
}

#[test]
fn the_decision_is_the_same_for_one_and_three_replicas_per_shard() {
    // decide_split takes no replica count: a shard's durable bytes do not
    // change with how many replicas serve it, so both inputs are the same.
    let one_replica = [usage(0, 500), usage(1, 20)];
    let three_replicas = [usage(0, 500), usage(1, 20)];
    let p = policy(100, None);

    assert_eq!(
        decide_split(2, p, &one_replica),
        decide_split(2, p, &three_replicas)
    );
    assert_eq!(
        decide_split(2, p, &one_replica),
        SplitDecision::Split {
            from_shard: 0,
            new_shard_count: 3
        }
    );
}

#[test]
fn a_bad_input_holds() {
    assert_eq!(decide_split(0, policy(100, None), &[]), SplitDecision::Hold);
    assert_eq!(
        decide_split(2, policy(0, None), &[usage(0, 5)]),
        SplitDecision::Hold
    );
    assert_eq!(
        decide_split(2, policy(100, None), &[usage(5, 500)]),
        SplitDecision::Hold
    );
    assert_eq!(
        decide_split(3, policy(100, Some(2)), &[usage(0, 500)]),
        SplitDecision::Hold
    );
}

#[test]
fn no_usage_holds() {
    assert_eq!(decide_split(2, policy(100, None), &[]), SplitDecision::Hold);
}

struct Fake;

impl BucketData for Fake {
    fn usage_bytes(&self, _shard: u32) -> Result<u64, BucketDataError> {
        Ok(7)
    }
    fn fence(&self, _shard: u32, _bucket: u32) -> Result<(), BucketDataError> {
        Ok(())
    }
    fn copy_pass(
        &self,
        _from: u32,
        _to: u32,
        _bucket: u32,
        _pass: CopyPass,
    ) -> Result<CopyReport, BucketDataError> {
        Ok(CopyReport { keys: 1, bytes: 2 })
    }
    fn prune(&self, _shard: u32, _bucket: u32) -> Result<(), BucketDataError> {
        Ok(())
    }
    fn checkpoint(&self, _shard: u32) -> Result<(), BucketDataError> {
        Ok(())
    }
    fn unfence(&self, _shard: u32, _bucket: u32) -> Result<(), BucketDataError> {
        Ok(())
    }
}

#[test]
fn bucket_data_is_object_safe_and_every_method_is_callable() {
    let hook: &dyn BucketData = &Fake;

    assert_eq!(hook.usage_bytes(0), Ok(7));
    assert_eq!(
        hook.copy_pass(0, 1, 3, CopyPass::First),
        Ok(CopyReport { keys: 1, bytes: 2 })
    );
    assert_eq!(hook.fence(0, 3), Ok(()));
    assert_eq!(hook.prune(0, 3), Ok(()));
    assert_eq!(hook.checkpoint(0), Ok(()));
    assert_eq!(hook.unfence(0, 3), Ok(()));
}

#[test]
fn the_error_messages_name_the_shard_and_the_detail() {
    assert_eq!(
        BucketDataError::Unreachable {
            shard: 2,
            detail: "timeout".to_string(),
        }
        .to_string(),
        "shard 2 is not reachable: timeout"
    );
    assert_eq!(
        BucketDataError::Refused {
            detail: "fenced".to_string(),
        }
        .to_string(),
        "the service refused the step: fenced"
    );
}
