use std::fmt;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a Google credential did not resolve to an authorized principal.
///
/// Every variant is distinguishable on purpose: "expired" and "wrong
/// audience" and "we could not reach Google" are three different operator
/// actions, and collapsing them into one 401 is what makes an auth layer
/// unsupportable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoogleAuthError {
    /// Not a well-formed JWT, or a JWT without the `kid` that key rotation
    /// makes mandatory.
    MalformedToken(String),
    /// The token is well formed but failed validation.
    Invalid(InvalidReason),
    /// The `kid` is absent from the JWKS and a refetch is rate-limited or did
    /// not produce it. Distinct from [`Self::SigningKeyUnavailable`]: here
    /// Google answered, and the key genuinely is not published.
    UnknownSigningKey { kid: String },
    /// The JWKS could not be fetched or parsed. An upstream problem, not a
    /// caller problem.
    SigningKeyUnavailable(String),
    /// The credential verified but carries no `email` claim — an ID token
    /// minted without `--include-email`.
    EmailMissing,
    /// The credential carries an email Google has not verified.
    EmailUnverified,
    /// The introspection endpoint answered that the token is not valid.
    Rejected,
    /// The introspection endpoint could not be reached. Distinct from
    /// [`Self::Rejected`], which is the whole point of the variant.
    IntrospectionUnavailable(String),
    /// No introspector is configured, so an opaque credential that missed the
    /// registry has nowhere left to go.
    IntrospectionNotConfigured,
    /// Authentication succeeded and authorization did not: a verified identity
    /// that no registry entry grants anything to.
    NotInRegistry,
}

/// The specific validation that failed. Separate from [`GoogleAuthError`] so
/// the five negatives an operator actually hits stay individually assertable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidReason {
    Audience,
    Issuer,
    Expired,
    Signature,
}

impl fmt::Display for GoogleAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedToken(why) => write!(f, "malformed credential: {why}"),
            Self::Invalid(InvalidReason::Audience) => {
                write!(f, "token was minted for a different audience")
            }
            Self::Invalid(InvalidReason::Issuer) => write!(f, "token issuer is not Google"),
            Self::Invalid(InvalidReason::Expired) => write!(f, "token has expired"),
            Self::Invalid(InvalidReason::Signature) => write!(f, "token signature is not valid"),
            Self::UnknownSigningKey { kid } => {
                write!(f, "signing key `{kid}` is not published by Google")
            }
            Self::SigningKeyUnavailable(why) => write!(f, "could not obtain signing keys: {why}"),
            Self::EmailMissing => write!(f, "token carries no email claim"),
            Self::EmailUnverified => write!(f, "token carries an unverified email"),
            Self::Rejected => write!(f, "credential was rejected by Google"),
            Self::IntrospectionUnavailable(why) => {
                write!(f, "could not reach Google to check the credential: {why}")
            }
            Self::IntrospectionNotConfigured => {
                write!(f, "no access-token introspection is configured")
            }
            Self::NotInRegistry => write!(f, "identity is not granted anything by the registry"),
        }
    }
}

impl std::error::Error for GoogleAuthError {}

impl GoogleAuthError {
    /// Whether this is an upstream failure rather than a verdict on the
    /// caller's credential. Drives the 503-vs-401 split.
    pub fn is_upstream_failure(&self) -> bool {
        matches!(
            self,
            Self::SigningKeyUnavailable(_) | Self::IntrospectionUnavailable(_)
        )
    }
}
