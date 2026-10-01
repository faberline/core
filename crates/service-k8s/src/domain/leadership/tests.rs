use super::may_acquire;

#[test]
fn unheld_lease_is_acquirable() {
    assert!(may_acquire(None, None, 15, "me", 1000));
}

#[test]
fn own_lease_is_renewable_even_when_fresh() {
    assert!(may_acquire(Some("me"), Some(999), 15, "me", 1000));
}

#[test]
fn other_holder_fresh_lease_blocks() {
    assert!(!may_acquire(Some("other"), Some(999), 15, "me", 1000));
}

#[test]
fn other_holder_expired_lease_is_taken_over() {
    assert!(may_acquire(Some("other"), Some(980), 15, "me", 1000));
    // exactly at the boundary (== duration) is NOT yet expired.
    assert!(!may_acquire(Some("other"), Some(985), 15, "me", 1000));
}

#[test]
fn other_holder_missing_renew_time_is_acquirable() {
    assert!(may_acquire(Some("other"), None, 15, "me", 1000));
}
