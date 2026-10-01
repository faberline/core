use std::time::Duration;

/// Both issuer spellings Google emits for ID tokens. Pinned, not configurable:
/// there is no second issuer to validate an abstraction against.
pub const GOOGLE_ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];

/// Google's published JWKS for ID-token signing keys.
pub const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

/// Google's access-token introspection endpoint.
pub const GOOGLE_TOKENINFO_URL: &str = "https://oauth2.googleapis.com/tokeninfo";

/// Floor between JWKS refetches. A caller presenting fabricated `kid` values
/// gets at most one upstream fetch per window, not one per request.
pub const DEFAULT_JWKS_REFETCH_MIN_INTERVAL: Duration = Duration::from_secs(60);

/// Ceiling on how long an introspection result is trusted.
///
/// This is the revocation-latency knob. Caching to the token's own
/// `expires_in` means a revoked credential keeps working for up to its
/// remaining hour; a short ceiling narrows that window at the cost of more
/// calls to Google.
pub const DEFAULT_INTROSPECTION_TTL_CEILING: Duration = Duration::from_secs(300);

/// Tunables for [`GoogleVerifier`].
#[derive(Debug, Clone)]
pub struct GoogleAuthConfig {
    /// Audiences an ID token may be minted for. An empty list is rejected at
    /// construction: an ID-token verifier that accepts any audience accepts
    /// tokens minted for someone else's service.
    pub audiences: Vec<String>,
    pub jwks_refetch_min_interval: Duration,
    pub introspection_ttl_ceiling: Duration,
}

impl GoogleAuthConfig {
    pub fn new(audiences: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            audiences: audiences.into_iter().map(Into::into).collect(),
            jwks_refetch_min_interval: DEFAULT_JWKS_REFETCH_MIN_INTERVAL,
            introspection_ttl_ceiling: DEFAULT_INTROSPECTION_TTL_CEILING,
        }
    }
}
