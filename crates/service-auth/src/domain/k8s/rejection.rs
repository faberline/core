use super::principal::PrincipalRejection;

/// Why a credential was not accepted. A classification, never an echo of the
/// value: rejected usernames are routinely email addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRejection {
    /// No `Authorization: Bearer` credential was presented.
    MissingCredential,
    /// The token is valid, but not for this service. The single most important
    /// rejection here: it is what stops a token minted for kube-apiserver from
    /// being replayed against a delegating service.
    AudienceMismatch,
    /// The reviewed identity is not an acceptable caller.
    Principal(PrincipalRejection),
}

impl AuthRejection {
    /// A stable, credential-free token for logs and metrics.
    pub fn reason(self) -> &'static str {
        match self {
            Self::MissingCredential => "missing_credential",
            Self::AudienceMismatch => "audience_mismatch",
            Self::Principal(inner) => inner.reason(),
        }
    }
}

impl std::fmt::Display for AuthRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}
