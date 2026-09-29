use std::time::Duration;

use crate::domain::k8s::ProjectedToken;

// ---------------------------------------------------------------------------
// The answer
// ---------------------------------------------------------------------------

/// Refresh with a fifth of the lifetime still to go — the same 80% mark the
/// kubelet uses for projected volumes, for the same reason: leave enough tail
/// that a slow mint, a retried request, or a clock a little out of step still
/// lands inside the window.
const REFRESH_TAIL_DIVISOR: u64 = 5;

/// Never run a token closer to its expiry than this, however short the
/// lifetime turned out to be.
const MIN_GUARD: Duration = Duration::from_secs(30);

/// A token and the moment the *server* said it stops working.
///
/// The expiry is not the one that was requested. `--service-account-max-token-
/// expiration` caps it cluster-wide, and a bound token is capped by the
/// lifetime of what it is bound to. A client that assumes it got what it asked
/// for works until someone lowers that flag.
#[derive(Debug, Clone)]
pub struct MintedToken {
    token: ProjectedToken,
    minted_at_millis: u64,
    expires_at_millis: u64,
}

impl MintedToken {
    pub fn new(token: impl Into<String>, minted_at_millis: u64, expires_at_millis: u64) -> Self {
        Self {
            token: ProjectedToken::new(token.into()),
            minted_at_millis,
            expires_at_millis,
        }
    }

    /// The credential. The only accessor that yields the material, and the
    /// wrapper it comes in refuses to print itself.
    pub fn token(&self) -> &ProjectedToken {
        &self.token
    }

    pub fn expires_at_millis(&self) -> u64 {
        self.expires_at_millis
    }

    /// When this token should be replaced: four fifths of the way through its
    /// life, or [`MIN_GUARD`] before the end, whichever comes first.
    pub fn refresh_at_millis(&self) -> u64 {
        let lifetime = self.expires_at_millis.saturating_sub(self.minted_at_millis);
        // The tail left unused: one fifth, but never less than the guard, and
        // never more than the whole lifetime — a token shorter than the guard
        // is due the moment it arrives rather than never.
        let tail = lifetime / REFRESH_TAIL_DIVISOR;
        let guard = tail.max(MIN_GUARD.as_millis() as u64).min(lifetime);
        self.expires_at_millis.saturating_sub(guard)
    }
}
