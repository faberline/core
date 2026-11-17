use super::*;

const MINUTE: Duration = Duration::from_secs(60);

#[test]
fn a_write_to_a_fenced_bucket_is_blocked_and_a_write_to_another_bucket_is_not() {
    let fence = WriteFence::new();
    let t0 = Instant::now();
    fence.arm(7, MINUTE, t0);
    assert!(fence.blocks(7, t0));
    assert!(!fence.blocks(8, t0));
}

#[test]
fn blocks_any_is_true_when_one_bucket_is_fenced() {
    let fence = WriteFence::new();
    let t0 = Instant::now();
    fence.arm(7, MINUTE, t0);
    assert!(fence.blocks_any(&[1, 7, 9], t0));
    assert!(!fence.blocks_any(&[1, 9], t0));
    assert!(!fence.blocks_any(&[], t0));
}

#[test]
fn the_fence_expires_after_its_ttl() {
    let fence = WriteFence::new();
    let t0 = Instant::now();
    fence.arm(7, MINUTE, t0);
    assert!(fence.blocks(7, t0 + MINUTE - Duration::from_millis(1)));
    assert!(!fence.blocks(7, t0 + MINUTE));
    assert!(!fence.blocks_any(&[7], t0 + MINUTE));
}

#[test]
fn clear_removes_the_fence() {
    let fence = WriteFence::new();
    let t0 = Instant::now();
    fence.arm(7, MINUTE, t0);
    assert!(fence.clear(7));
    assert!(!fence.blocks(7, t0));
    assert!(!fence.clear(7));
}

#[test]
fn a_ttl_above_the_maximum_is_cut_to_3600_seconds() {
    let fence = WriteFence::new();
    let t0 = Instant::now();
    fence.arm(7, Duration::from_secs(86_400), t0);
    assert!(fence.blocks(7, t0 + Duration::from_secs(3599)));
    assert!(!fence.blocks(7, t0 + Duration::from_secs(3600)));
    assert_eq!(MAX_FENCE_TTL, Duration::from_secs(3600));
}

#[test]
fn arming_again_replaces_the_end_time() {
    let fence = WriteFence::new();
    let t0 = Instant::now();
    fence.arm(7, MINUTE, t0);
    fence.arm(7, 2 * MINUTE, t0);
    assert!(fence.blocks(7, t0 + MINUTE));
    assert!(!fence.blocks(7, t0 + 2 * MINUTE));
}
