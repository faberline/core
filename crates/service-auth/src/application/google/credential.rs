use jsonwebtoken::{decode_header, Algorithm};

/// Which shape a presented credential has, and therefore which path verifies
/// it. Selection never falls through from one path to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credential {
    /// A three-segment RS256 JWT carrying a `kid` — the offline path.
    GoogleIdToken,
    /// Anything else — registry lookup, then introspection.
    Opaque,
}

/// Classify a presented credential by shape alone.
pub fn classify(token: &str) -> Credential {
    if token.split('.').count() != 3 {
        return Credential::Opaque;
    }
    match decode_header(token) {
        Ok(header) if header.kid.is_some() && header.alg == Algorithm::RS256 => {
            Credential::GoogleIdToken
        }
        _ => Credential::Opaque,
    }
}
