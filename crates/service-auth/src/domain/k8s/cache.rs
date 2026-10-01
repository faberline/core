use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Monotonically-read wall time, in milliseconds.
///
/// Injectable because the properties worth testing here are all temporal:
/// "a revoked allow stops working within six minutes" is only a test if time
/// can be moved without waiting for it.
pub trait Clock: Send + Sync + 'static {
    fn now_millis(&self) -> u64;
}

/// A clock a test drives by hand.
///
/// Public because the services that build on this cache have to prove their own
/// revocation bounds, and a private test clock would force each of them to
/// reinvent one.
#[derive(Debug, Default)]
pub struct ManualClock {
    millis: AtomicU64,
}

impl ManualClock {
    pub fn new(millis: u64) -> Self {
        Self {
            millis: AtomicU64::new(millis),
        }
    }

    pub fn advance(&self, by: Duration) {
        self.millis
            .fetch_add(by.as_millis() as u64, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_millis(&self) -> u64 {
        self.millis.load(Ordering::SeqCst)
    }
}

/// How long each class of answer is reusable.
///
/// Start from [`CachePolicy::default`] (allow 300 s, deny 30 s, stale window
/// 60 s, 8192 entries) and change what differs with the `with_*` builders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachePolicy {
    allow_ttl: Duration,
    deny_ttl: Duration,
    stale_window: Duration,
    max_entries: usize,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            allow_ttl: Duration::from_secs(300),
            deny_ttl: Duration::from_secs(30),
            stale_window: Duration::from_secs(60),
            max_entries: 8192,
        }
    }
}

impl CachePolicy {
    /// Set how long a successful authentication or an allow may be reused.
    pub fn with_allow_ttl(mut self, allow_ttl: Duration) -> Self {
        self.allow_ttl = allow_ttl;
        self
    }

    /// Set how long a deny may be reused.
    pub fn with_deny_ttl(mut self, deny_ttl: Duration) -> Self {
        self.deny_ttl = deny_ttl;
        self
    }

    /// Set how far past its TTL an entry may be served when the apiserver
    /// failed to answer. Zero disables serving stale entries.
    pub fn with_stale_window(mut self, stale_window: Duration) -> Self {
        self.stale_window = stale_window;
        self
    }

    /// Set the hard ceiling on retained entries.
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// How long a successful authentication or an allow may be reused.
    pub fn allow_ttl(&self) -> Duration {
        self.allow_ttl
    }

    /// How long a deny may be reused. Short on purpose: this TTL is the delay
    /// between granting access and access working.
    pub fn deny_ttl(&self) -> Duration {
        self.deny_ttl
    }

    /// How far past its TTL an entry may be served *only* when the apiserver
    /// failed to answer. Zero disables serving stale entries entirely.
    pub fn stale_window(&self) -> Duration {
        self.stale_window
    }

    /// The hard ceiling on retained entries. An unauthenticated caller chooses
    /// the token, and therefore the key, so an unbounded map is a memory
    /// exhaustion primitive handed to anyone who can reach the port.
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// The worst-case delay between a revocation taking effect in Kubernetes
    /// and this process refusing the caller.
    pub fn revocation_bound(&self) -> Duration {
        self.allow_ttl + self.stale_window
    }
}

/// What a lookup found, and how much it should be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheOutcome {
    /// Served from an unexpired entry.
    Hit,
    /// Nothing usable; the caller must ask the apiserver.
    Miss,
    /// Served from an expired entry because the apiserver could not be
    /// reached, and the entry is still inside the stale window.
    Stale,
}

impl CacheOutcome {
    /// A stable label for metrics.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::Stale => "stale",
        }
    }
}

#[derive(Debug, Clone)]
struct Entry<V> {
    value: V,
    /// Absolute expiry. Past this, only [`TtlCache::get_stale`] can see it.
    expires_at_millis: u64,
    /// Absolute end of the stale window. Past this the entry is unreachable.
    discard_at_millis: u64,
}

/// A bounded, TTL-keyed cache with a separate stale-on-failure read path.
pub struct TtlCache<K, V> {
    entries: Mutex<HashMap<K, Entry<V>>>,
    policy: CachePolicy,
    clock: Arc<dyn Clock>,
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(policy: CachePolicy, clock: Arc<dyn Clock>) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            policy,
            clock,
        }
    }

    pub fn policy(&self) -> &CachePolicy {
        &self.policy
    }

    /// Store a value under a TTL chosen by the caller — `allow_ttl` or
    /// `deny_ttl`, depending on what the answer was.
    pub fn insert(&self, key: K, value: V, ttl: Duration) {
        let now = self.clock.now_millis();
        let expires_at_millis = now.saturating_add(ttl.as_millis() as u64);
        let discard_at_millis =
            expires_at_millis.saturating_add(self.policy.stale_window.as_millis() as u64);
        let mut entries = self.lock();
        entries.insert(
            key,
            Entry {
                value,
                expires_at_millis,
                discard_at_millis,
            },
        );
        Self::enforce_capacity(&mut entries, self.policy.max_entries, now);
    }

    /// The fast path: an unexpired entry, or nothing. Never returns an expired
    /// entry, so no caller can drift into serving stale data by accident.
    pub fn get(&self, key: &K) -> Option<V> {
        let now = self.clock.now_millis();
        let mut entries = self.lock();
        let entry = entries.get(key)?;
        if now < entry.expires_at_millis {
            return Some(entry.value.clone());
        }
        if now >= entry.discard_at_millis {
            entries.remove(key);
        }
        None
    }

    /// The outage path: an entry that is past its TTL but still inside the
    /// stale window. Only legitimate after a review actually failed — calling
    /// it otherwise silently widens the revocation bound.
    pub fn get_stale(&self, key: &K) -> Option<V> {
        let now = self.clock.now_millis();
        let mut entries = self.lock();
        let entry = entries.get(key)?;
        if now < entry.discard_at_millis {
            return Some(entry.value.clone());
        }
        entries.remove(key);
        None
    }

    pub fn remove(&self, key: &K) {
        self.lock().remove(key);
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Retained entries, including ones only `get_stale` can still see.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A poisoned mutex here means a panic inside a cache method, which cannot
    /// leave a `HashMap` in a state that threatens correctness. Recovering
    /// beats propagating a panic into every subsequent request.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<K, Entry<V>>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Drop fully-discardable entries first; only if that is not enough does
    /// the cache evict live ones, oldest expiry first.
    fn enforce_capacity(entries: &mut HashMap<K, Entry<V>>, max_entries: usize, now: u64) {
        if entries.len() <= max_entries {
            return;
        }
        entries.retain(|_, entry| now < entry.discard_at_millis);
        while entries.len() > max_entries {
            let Some(victim) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at_millis)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            entries.remove(&victim);
        }
    }
}

#[cfg(test)]
mod tests;
