use std::fmt;

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

/// Why no token was minted. No variant carries a token, and none carries the
/// caller's own credential either — the exec plugin's output is a credential
/// too, and `kube`'s error for a failed plugin is one of the places it can
/// surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRequestError {
    /// Refused before anything was sent.
    InvalidTarget {
        field: &'static str,
        value: String,
        reason: String,
    },
    /// No usable Kubernetes client configuration, or the apiserver would not
    /// authenticate the caller at all.
    NoIdentity { detail: String },
    /// The caller authenticated, and RBAC said no.
    ///
    /// `username` is what the apiserver says the caller is — resolved by
    /// asking it, not parsed out of the denial text. `None` means even that
    /// question failed, which is worth saying rather than guessing.
    Forbidden {
        username: Option<String>,
        namespace: String,
        service_account: String,
        detail: String,
    },
    /// The ServiceAccount does not exist. Distinct from [`Self::Forbidden`] on
    /// purpose: one is a grant to fix, the other is a name to fix, and a
    /// `create` grant that names a ServiceAccount nobody created reads as the
    /// first while being the second.
    NoSuchServiceAccount {
        namespace: String,
        service_account: String,
    },
    /// The round trip did not complete.
    Transport { detail: String },
    /// It completed and the answer was not one.
    Malformed { detail: String },
}

impl TokenRequestError {
    /// The `kubectl` question whose answer is this error, for callers
    /// assembling remediation. Names the ServiceAccount, so it asks about the
    /// grant that was actually missing rather than the namespace-wide one.
    pub fn can_i_command(namespace: &str, service_account: &str) -> String {
        format!(
            "kubectl auth can-i create serviceaccounts/{service_account} --subresource=token \
             -n {namespace}"
        )
    }
}

impl fmt::Display for TokenRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget {
                field,
                value,
                reason,
            } => write!(f, "invalid {field} `{value}`: {reason}"),
            Self::NoIdentity { detail } => write!(
                f,
                "no Kubernetes identity to mint a token with: {detail} — this path uses your \
                 kubeconfig and nothing else, so `kubectl auth whoami` failing here means the \
                 same thing it would there"
            ),
            Self::Forbidden {
                username,
                namespace,
                service_account,
                detail,
            } => {
                let who = match username {
                    Some(name) => format!("`{name}`"),
                    None => "the identity in your kubeconfig".to_string(),
                };
                write!(
                    f,
                    "{who} may not mint a token for ServiceAccount `{namespace}/{service_account}`: \
                     {detail}. Check with `{}`; the missing grant is `create` on \
                     `serviceaccounts/token` with `resourceNames: [{service_account}]`",
                    Self::can_i_command(namespace, service_account)
                )
            }
            Self::NoSuchServiceAccount {
                namespace,
                service_account,
            } => write!(
                f,
                "ServiceAccount `{namespace}/{service_account}` does not exist — a token can only \
                 be minted for an account that does, and a grant naming one that does not is \
                 accepted by RBAC without ever working"
            ),
            Self::Transport { detail } => {
                write!(f, "the TokenRequest did not complete: {detail}")
            }
            Self::Malformed { detail } => write!(
                f,
                "the apiserver accepted the TokenRequest and did not answer it: {detail}"
            ),
        }
    }
}

impl std::error::Error for TokenRequestError {}
