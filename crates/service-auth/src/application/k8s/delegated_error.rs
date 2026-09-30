use crate::application::http::AuthError;
use crate::domain::k8s::{AuthRejection, ResourceAttributes, ReviewError};

/// The three outcomes a delegated check can have, kept distinct all the way to
/// the HTTP layer.
///
/// Collapsing [`Unavailable`](Self::Unavailable) into a deny would be safe but
/// dishonest — it tells an operator their RBAC is wrong when their apiserver is
/// unreachable. Collapsing it into an allow is the failure this design exists
/// to prevent.
#[derive(Debug, Clone)]
pub enum DelegatedAuthError {
    /// The caller is not who they need to be — 401.
    Unauthenticated(AuthRejection),
    /// The caller is known and not permitted — 403.
    Denied(ResourceAttributes),
    /// No decision could be reached — 503.
    Unavailable(ReviewError),
}

impl DelegatedAuthError {
    /// A stable, credential-free token for logs and metrics.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Unauthenticated(rejection) => rejection.reason(),
            Self::Denied(_) => "denied",
            Self::Unavailable(error) => error.reason(),
        }
    }
}

impl std::fmt::Display for DelegatedAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated(rejection) => write!(f, "unauthenticated: {rejection}"),
            Self::Denied(attributes) => write!(f, "not permitted to {attributes}"),
            Self::Unavailable(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DelegatedAuthError {}

impl From<DelegatedAuthError> for AuthError {
    fn from(error: DelegatedAuthError) -> Self {
        match error {
            // The rejection reason is a classification, so it is safe to
            // return; it tells an operator reading a client's logs which of
            // the several 401 conditions they hit.
            DelegatedAuthError::Unauthenticated(_) => AuthError::Unauthenticated,
            DelegatedAuthError::Denied(attributes) => {
                AuthError::Forbidden(format!("not permitted to {attributes}"))
            }
            DelegatedAuthError::Unavailable(_) => {
                AuthError::Unavailable("authorization is temporarily unavailable".into())
            }
        }
    }
}
