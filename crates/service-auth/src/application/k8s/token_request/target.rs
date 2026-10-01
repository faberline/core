use serde_json::json;

use super::error::TokenRequestError;

/// The lifetime to ask for when the caller has no reason to prefer another.
///
/// Also the apiserver's floor: `TokenRequestSpec.expirationSeconds` below ten
/// minutes is rejected by validation, so this is simultaneously "short" and
/// "the shortest thing that works".
pub const DEFAULT_EXPIRATION_SECONDS: i64 = 600;

/// Below this, kube-apiserver refuses the request outright.
pub const MIN_EXPIRATION_SECONDS: i64 = 600;

// ---------------------------------------------------------------------------
// The request
// ---------------------------------------------------------------------------

/// Which ServiceAccount's token to ask for, for whom, and for how long.
///
/// Constructing one validates the names. That is not tidiness: `namespace` and
/// `service_account` are interpolated into a URL path, and a value containing
/// `/` or `..` would address a different resource than the one the caller
/// named — which is the one thing an explicit-target contract must not allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRequestTarget {
    namespace: String,
    service_account: String,
    audience: String,
    expiration_seconds: i64,
}

impl TokenRequestTarget {
    /// A target for the named ServiceAccount, at [`DEFAULT_EXPIRATION_SECONDS`].
    pub fn new(
        namespace: impl Into<String>,
        service_account: impl Into<String>,
        audience: impl Into<String>,
    ) -> Result<Self, TokenRequestError> {
        let namespace = namespace.into();
        let service_account = service_account.into();
        let audience = audience.into();
        check_object_name("namespace", &namespace)?;
        check_object_name("client service account", &service_account)?;
        if audience.trim().is_empty() {
            return Err(TokenRequestError::InvalidTarget {
                field: "audience",
                value: audience,
                reason: "an audience-bound token needs an audience; a token minted with none is \
                         accepted by every service that does not check, which is the failure \
                         this whole path exists to prevent"
                    .to_string(),
            });
        }
        Ok(Self {
            namespace,
            service_account,
            audience,
            expiration_seconds: DEFAULT_EXPIRATION_SECONDS,
        })
    }

    /// A target that asks kube-apiserver to use its configured default
    /// audiences. The empty `spec.audiences` array is meaningful here and is
    /// distinct from an explicit audience, which [`Self::new`] still requires.
    pub fn kubernetes_default(
        namespace: impl Into<String>,
        service_account: impl Into<String>,
    ) -> Result<Self, TokenRequestError> {
        let namespace = namespace.into();
        let service_account = service_account.into();
        check_object_name("namespace", &namespace)?;
        check_object_name("client service account", &service_account)?;
        Ok(Self {
            namespace,
            service_account,
            audience: String::new(),
            expiration_seconds: DEFAULT_EXPIRATION_SECONDS,
        })
    }

    /// Ask for a different lifetime. The apiserver may still issue a shorter
    /// one, and [`MintedToken`] carries what it actually issued.
    pub fn with_expiration_seconds(mut self, seconds: i64) -> Result<Self, TokenRequestError> {
        if seconds < MIN_EXPIRATION_SECONDS {
            return Err(TokenRequestError::InvalidTarget {
                field: "expiration",
                value: seconds.to_string(),
                reason: format!(
                    "kube-apiserver rejects a TokenRequest below {MIN_EXPIRATION_SECONDS} seconds"
                ),
            });
        }
        self.expiration_seconds = seconds;
        Ok(self)
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn service_account(&self) -> &str {
        &self.service_account
    }

    pub fn audience(&self) -> &str {
        &self.audience
    }

    pub fn expiration_seconds(&self) -> i64 {
        self.expiration_seconds
    }

    /// The subresource this request POSTs to.
    ///
    /// A `k8s`-gated test checks this against the path `kube` derives on its
    /// own, so the string here cannot quietly stop describing where the
    /// request goes.
    pub fn subresource_path(&self) -> String {
        format!(
            "/api/v1/namespaces/{}/serviceaccounts/{}/token",
            self.namespace, self.service_account
        )
    }

    /// The literal request body.
    ///
    /// [`KubeTokenMinter`] deserializes *this* into the typed `TokenRequest`
    /// rather than building a second one beside it, so the audience and
    /// duration a test asserts here are the audience and duration that go on
    /// the wire.
    pub fn request_body(&self) -> serde_json::Value {
        let audiences = if self.audience.is_empty() {
            json!([])
        } else {
            json!([self.audience])
        };
        json!({
            "apiVersion": "authentication.k8s.io/v1",
            "kind": "TokenRequest",
            "spec": {
                "audiences": audiences,
                "expirationSeconds": self.expiration_seconds,
            }
        })
    }
}

/// DNS-1123-ish: what Kubernetes accepts for a namespace or ServiceAccount
/// name, checked here so a path-bearing value is refused before it becomes a
/// URL. The message names the rule rather than restating the input, because
/// the input is what is already on screen.
fn check_object_name(field: &'static str, value: &str) -> Result<(), TokenRequestError> {
    let invalid = |reason: &str| TokenRequestError::InvalidTarget {
        field,
        value: value.to_string(),
        reason: reason.to_string(),
    };
    if value.is_empty() {
        return Err(invalid("a name is required; this target is never inferred"));
    }
    if value.len() > 253 {
        return Err(invalid("Kubernetes names stop at 253 characters"));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
    {
        return Err(invalid(
            "a Kubernetes name is lowercase alphanumerics, `-`, and `.` — nothing else, and in \
             particular no `/`, which would address a different object than the one named",
        ));
    }
    if !value.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        || !value.ends_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err(invalid(
            "a Kubernetes name starts and ends with a letter or a digit",
        ));
    }
    Ok(())
}
