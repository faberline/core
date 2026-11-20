//! In-memory `Idempotency-Key` store. A stored response stays valid for 24 hours.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// How long a stored response stays valid: 24 hours.
pub const IDEMPOTENCY_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// In-memory store of `Idempotency-Key` -> response. The caller passes the clock as `now`.
#[derive(Debug)]
pub struct IdempotencyStore<R> {
    entries: Mutex<HashMap<String, (Duration, R)>>,
}

impl<R: Clone> IdempotencyStore<R> {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// The stored response when the key exists and is still live at `now`.
    pub fn get(&self, key: &str, now: Duration) -> Option<R> {
        let mut entries = self.lock();
        let (stored_at, response) = entries.get(key)?;
        if now.saturating_sub(*stored_at) < IDEMPOTENCY_TTL {
            return Some(response.clone());
        }
        entries.remove(key);
        None
    }

    /// Store `response` for `key` at `now`. Returns `false` and keeps the entry when a live one exists.
    pub fn put(&self, key: &str, response: R, now: Duration) -> bool {
        let mut entries = self.lock();
        if let Some((stored_at, _)) = entries.get(key) {
            if now.saturating_sub(*stored_at) < IDEMPOTENCY_TTL {
                return false;
            }
        }
        entries.insert(key.to_string(), (now, response));
        true
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, (Duration, R)>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl<R: Clone> Default for IdempotencyStore<R> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn same_key_within_24_hours_returns_stored_response() {
        let store = IdempotencyStore::<String>::new();
        assert!(store.put("k", "a".to_string(), secs(100)));
        assert_eq!(
            store.get("k", secs(100) + IDEMPOTENCY_TTL - secs(1)),
            Some("a".to_string())
        );
        assert!(!store.put("k", "b".to_string(), secs(200)));
        assert_eq!(store.get("k", secs(200)), Some("a".to_string()));
    }

    #[test]
    fn key_is_free_after_24_hours() {
        let store = IdempotencyStore::<String>::new();
        assert!(store.put("k", "a".to_string(), secs(100)));
        assert_eq!(store.get("k", secs(100) + IDEMPOTENCY_TTL), None);
        assert!(store.put("k", "b".to_string(), secs(100) + IDEMPOTENCY_TTL));
        assert_eq!(
            store.get("k", secs(100) + IDEMPOTENCY_TTL),
            Some("b".to_string())
        );
    }

    #[test]
    fn different_keys_do_not_share_a_response() {
        let store = IdempotencyStore::<String>::new();
        assert!(store.put("k1", "a".to_string(), secs(0)));
        assert!(store.put("k2", "b".to_string(), secs(0)));
        assert_eq!(store.get("k1", secs(0)), Some("a".to_string()));
        assert_eq!(store.get("k2", secs(0)), Some("b".to_string()));
        assert_eq!(store.get("k3", secs(0)), None);
    }

    #[test]
    fn default_store_starts_empty() {
        let store = IdempotencyStore::<String>::default();
        assert_eq!(store.get("k", secs(0)), None);
    }
}
