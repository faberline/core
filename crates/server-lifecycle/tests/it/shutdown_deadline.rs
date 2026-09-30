use std::time::Duration;

use server_lifecycle::ShutdownDeadline;

#[tokio::test(start_paused = true)]
async fn one_absolute_deadline_preserves_reserve() {
    let deadline =
        ShutdownDeadline::from_now(Duration::from_millis(100), Duration::from_millis(20)).unwrap();
    assert_eq!(deadline.usable_remaining(), Duration::from_millis(80));
    tokio::time::advance(Duration::from_millis(60)).await;
    assert_eq!(deadline.usable_remaining(), Duration::from_millis(20));
    tokio::time::advance(Duration::from_millis(20)).await;
    assert_eq!(deadline.usable_remaining(), Duration::ZERO);
}

#[test]
fn reserve_cannot_exceed_total() {
    assert!(ShutdownDeadline::from_now(Duration::from_secs(1), Duration::from_secs(2)).is_err());
}

#[test]
fn new_keeps_a_fixed_expiry_and_checks_the_reserve() {
    let expires_at = tokio::time::Instant::now() + Duration::from_millis(100);
    let deadline = ShutdownDeadline::new(
        expires_at,
        Duration::from_millis(100),
        Duration::from_millis(20),
    )
    .unwrap();
    assert_eq!(deadline.expires_at(), expires_at);
    assert_eq!(deadline.total(), Duration::from_millis(100));
    assert_eq!(deadline.reserve(), Duration::from_millis(20));
    assert!(
        ShutdownDeadline::new(expires_at, Duration::from_secs(1), Duration::from_secs(2)).is_err()
    );
}
