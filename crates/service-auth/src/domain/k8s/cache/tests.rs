use std::sync::Arc;

use super::*;

fn cache(policy: CachePolicy) -> (TtlCache<String, bool>, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(1_000_000));
    (TtlCache::new(policy, clock.clone()), clock)
}

#[test]
fn a_fresh_entry_is_served_and_an_expired_one_is_not() {
    let (cache, clock) = cache(CachePolicy::default());
    cache.insert("k".into(), true, Duration::from_secs(300));
    assert_eq!(cache.get(&"k".to_string()), Some(true));

    clock.advance(Duration::from_secs(299));
    assert_eq!(
        cache.get(&"k".to_string()),
        Some(true),
        "one second before expiry the entry is still fresh"
    );

    clock.advance(Duration::from_secs(2));
    assert_eq!(
        cache.get(&"k".to_string()),
        None,
        "the fast path must never serve an expired entry"
    );
}

/// The stale window is only reachable through `get_stale`, which is what
/// makes "we cache" and "we fail closed" both true.
#[test]
fn an_expired_entry_is_reachable_only_through_the_outage_path() {
    let (cache, clock) = cache(CachePolicy::default());
    cache.insert("k".into(), true, Duration::from_secs(300));
    clock.advance(Duration::from_secs(301));

    assert_eq!(cache.get(&"k".to_string()), None);
    assert_eq!(
        cache.get_stale(&"k".to_string()),
        Some(true),
        "30s past expiry is inside the 60s stale window"
    );
}

/// AC6: the revocation bound is a number, and past it there is no path
/// back to the cached answer.
#[test]
fn past_the_stale_window_even_the_outage_path_fails_closed() {
    let policy = CachePolicy::default();
    let (cache, clock) = cache(policy);
    cache.insert("k".into(), true, policy.allow_ttl());

    clock.advance(policy.revocation_bound());
    clock.advance(Duration::from_millis(1));

    assert_eq!(cache.get(&"k".to_string()), None);
    assert_eq!(
        cache.get_stale(&"k".to_string()),
        None,
        "there is no read path that outlives allow_ttl + stale_window"
    );
    assert_eq!(
        policy.revocation_bound(),
        Duration::from_secs(360),
        "the documented worst case is six minutes"
    );
}

/// A deny gets its own, much shorter TTL: it delays a *grant* taking
/// effect, so long-lived denies are an availability problem.
#[test]
fn a_deny_expires_far_sooner_than_an_allow() {
    let policy = CachePolicy::default();
    let (cache, clock) = cache(policy);
    cache.insert("allow".into(), true, policy.allow_ttl());
    cache.insert("deny".into(), false, policy.deny_ttl());

    clock.advance(Duration::from_secs(31));
    assert_eq!(cache.get(&"deny".to_string()), None);
    assert_eq!(cache.get(&"allow".to_string()), Some(true));
}

#[test]
fn a_zero_stale_window_removes_the_outage_path_entirely() {
    let policy = CachePolicy::default().with_stale_window(Duration::ZERO);
    let (cache, clock) = cache(policy);
    cache.insert("k".into(), true, policy.allow_ttl());
    clock.advance(policy.allow_ttl());
    clock.advance(Duration::from_millis(1));

    assert_eq!(cache.get_stale(&"k".to_string()), None);
    assert_eq!(policy.revocation_bound(), policy.allow_ttl());
}

/// The key is caller-chosen, so the ceiling has to hold under a flood.
#[test]
fn the_entry_ceiling_holds_against_an_unbounded_key_space() {
    let policy = CachePolicy::default().with_max_entries(16);
    let (cache, _clock) = cache(policy);
    for i in 0..1_000 {
        cache.insert(format!("k{i}"), true, Duration::from_secs(300));
    }
    assert!(
        cache.len() <= 16,
        "cache grew to {} entries past a ceiling of 16",
        cache.len()
    );
}

#[test]
fn eviction_prefers_entries_nothing_can_read_any_more() {
    let policy = CachePolicy::default().with_max_entries(2);
    let (cache, clock) = cache(policy);
    cache.insert("unreadable".into(), true, Duration::from_secs(1));
    clock.advance(Duration::from_secs(120));

    cache.insert("a".into(), true, Duration::from_secs(300));
    clock.advance(Duration::from_secs(1));
    cache.insert("b".into(), true, Duration::from_secs(300));

    assert_eq!(
        cache.get_stale(&"unreadable".to_string()),
        None,
        "the entry past its stale window is the one the ceiling should reclaim"
    );
    assert_eq!(cache.get(&"a".to_string()), Some(true));
    assert_eq!(cache.get(&"b".to_string()), Some(true));

    // With nothing discardable left, a third entry costs the entry that
    // expires soonest — the one whose remaining value is lowest.
    clock.advance(Duration::from_secs(1));
    cache.insert("c".into(), true, Duration::from_secs(300));
    assert_eq!(cache.get(&"a".to_string()), None);
    assert_eq!(cache.get(&"b".to_string()), Some(true));
    assert_eq!(cache.get(&"c".to_string()), Some(true));
}

#[test]
fn outcome_labels_are_stable() {
    assert_eq!(CacheOutcome::Hit.label(), "hit");
    assert_eq!(CacheOutcome::Miss.label(), "miss");
    assert_eq!(CacheOutcome::Stale.label(), "stale");
}
