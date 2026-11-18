use super::*;

#[test]
fn two_equal_maps_give_no_moves() {
    let a = VirtualBucketShardMap::balanced(1, 12, 3).unwrap();
    let b = VirtualBucketShardMap::balanced(2, 12, 3).unwrap();
    assert_eq!(bucket_moves(&a, &a).unwrap(), Vec::<BucketMove>::new());
    assert_eq!(bucket_moves(&a, &b).unwrap(), Vec::<BucketMove>::new());
}

#[test]
fn bucket_moves_reports_only_reassigned_buckets() {
    let from = VirtualBucketShardMap::new(1, vec![0, 0, 1, 1], 2).unwrap();
    let to = VirtualBucketShardMap::new(2, vec![0, 1, 1, 0], 2).unwrap();
    assert_eq!(
        bucket_moves(&from, &to).unwrap(),
        vec![
            BucketMove {
                bucket: 1,
                from_shard: 0,
                to_shard: 1
            },
            BucketMove {
                bucket: 3,
                from_shard: 1,
                to_shard: 0
            },
        ]
    );
}

#[test]
fn a_split_moves_only_from_existing_shards_to_the_new_shard() {
    let old = VirtualBucketShardMap::balanced(0, 12, 3).unwrap();
    let new = old.split_one_shard(1).unwrap();
    let moves = bucket_moves(&old, &new).unwrap();
    assert_eq!(moves.len(), 3);
    assert!(moves.iter().all(|m| m.to_shard == 3 && m.from_shard < 3));
    let buckets: Vec<u32> = moves.iter().map(|m| m.bucket).collect();
    assert_eq!(buckets, vec![0, 1, 2]);
    let froms: Vec<u32> = moves.iter().map(|m| m.from_shard).collect();
    assert_eq!(froms, vec![0, 1, 2]);
}

#[test]
fn each_moved_bucket_appears_once_and_the_list_is_sorted() {
    let old = VirtualBucketShardMap::balanced(0, 4096, 5).unwrap();
    let new = old.split_one_shard(1).unwrap();
    let moves = bucket_moves(&old, &new).unwrap();
    assert!(!moves.is_empty());
    assert!(moves.windows(2).all(|w| w[0].bucket < w[1].bucket));
    let unique: std::collections::BTreeSet<u32> = moves.iter().map(|m| m.bucket).collect();
    assert_eq!(unique.len(), moves.len());
    let expected = (0..4096u32)
        .filter(|&b| old.assignment_for_bucket(b) != new.assignment_for_bucket(b))
        .count();
    assert_eq!(moves.len(), expected);
}

#[test]
fn maps_with_different_bucket_counts_are_refused() {
    let a = VirtualBucketShardMap::balanced(1, 4, 2).unwrap();
    let b = VirtualBucketShardMap::balanced(1, 8, 2).unwrap();
    let err = bucket_moves(&a, &b).unwrap_err().to_string();
    assert_eq!(
        err,
        "shard maps have different virtual bucket counts: 4 and 8"
    );
}

#[test]
fn single_map_to_two_shards_moves_nothing_when_bucket_stays() {
    let old = VirtualBucketShardMap::single();
    let new = VirtualBucketShardMap::single().split_one_shard(1).unwrap();
    assert_eq!(bucket_moves(&old, &new).unwrap(), Vec::<BucketMove>::new());
}
