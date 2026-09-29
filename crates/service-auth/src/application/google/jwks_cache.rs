use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use jsonwebtoken::{jwk::JwkSet, DecodingKey};

use super::jwks_source::JwksSource;
use crate::domain::google::{Clock, GoogleAuthError};

// ---------------------------------------------------------------------------
// JWKS cache
// ---------------------------------------------------------------------------

#[derive(Default)]
struct JwksState {
    keys: Option<JwkSet>,
    last_fetch_unix: Option<u64>,
}

/// Caches Google's signing keys and refetches, at a bounded rate, when a `kid`
/// misses.
pub struct JwksCache {
    source: Arc<dyn JwksSource>,
    clock: Arc<dyn Clock>,
    min_refetch_interval: Duration,
    state: RwLock<JwksState>,
}

impl fmt::Debug for JwksCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JwksCache")
            .field("min_refetch_interval", &self.min_refetch_interval)
            .finish_non_exhaustive()
    }
}

impl JwksCache {
    pub fn new(
        source: Arc<dyn JwksSource>,
        clock: Arc<dyn Clock>,
        min_refetch_interval: Duration,
    ) -> Self {
        Self {
            source,
            clock,
            min_refetch_interval,
            state: RwLock::new(JwksState::default()),
        }
    }

    fn cached_key(&self, kid: &str) -> Result<Option<DecodingKey>, GoogleAuthError> {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(keys) = state.keys.as_ref() else {
            return Ok(None);
        };
        let Some(jwk) = keys.find(kid) else {
            return Ok(None);
        };
        DecodingKey::from_jwk(jwk).map(Some).map_err(|e| {
            GoogleAuthError::SigningKeyUnavailable(format!(
                "JWKS entry for `{kid}` is not a usable key: {e}"
            ))
        })
    }

    /// Resolve a `kid` to a decoding key, refetching at most once per window.
    ///
    /// The lock is never held across the fetch, so a slow upstream cannot
    /// stall requests that hit the cache. Two concurrent misses can therefore
    /// both fetch; that is bounded and preferable to serializing every request
    /// behind one lock.
    pub async fn decoding_key(&self, kid: &str) -> Result<DecodingKey, GoogleAuthError> {
        if let Some(key) = self.cached_key(kid)? {
            return Ok(key);
        }

        let now = self.clock.now_unix();
        {
            let state = self
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(last) = state.last_fetch_unix {
                if now.saturating_sub(last) < self.min_refetch_interval.as_secs() {
                    return Err(GoogleAuthError::UnknownSigningKey {
                        kid: kid.to_string(),
                    });
                }
            }
        }

        let fetched = self.source.fetch().await;
        {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.last_fetch_unix = Some(now);
            match fetched {
                Ok(keys) => state.keys = Some(keys),
                Err(why) => return Err(GoogleAuthError::SigningKeyUnavailable(why)),
            }
        }

        self.cached_key(kid)?
            .ok_or_else(|| GoogleAuthError::UnknownSigningKey {
                kid: kid.to_string(),
            })
    }
}
