// ---------------------------------------------------------------------------
// Injection seams: clock, JWKS source, introspection
// ---------------------------------------------------------------------------

/// Wall clock, injected so cache expiry and refetch rate limiting are testable
/// without sleeping.
///
/// JWT `exp` validation is **not** routed through this — `jsonwebtoken` reads
/// the system clock itself, so expiry tests mint a token with a past `exp`
/// rather than moving a fake clock.
pub trait Clock: Send + Sync {
    fn now_unix(&self) -> u64;
}
