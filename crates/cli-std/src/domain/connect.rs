use std::collections::HashMap;

use serde::Deserialize;

/// The `token-registry.json` key every token-registry Secret stores its
/// payload under (see `lumen llm --topic auth`'s Secret shape).
pub const TOKEN_REGISTRY_SECRET_KEY: &str = "token-registry.json";

/// Role hierarchy for token-registry Secret coverage checks: `Admin` ⊇
/// `Write` ⊇ `Read`. Structurally mirrors `service_auth::Role`; kept
/// independent here (rather than depending on `service-auth`) because
/// `service-auth` itself depends on `cli-std` — depending back would be a
/// dependency cycle. Adopters that already carry `service_auth::Role`
/// convert between the two at the call site (their own "role mapping", per
/// the `connect` adapter convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Read,
    Write,
    Admin,
}

impl Role {
    /// Whether this role meets or exceeds `needed`.
    pub fn covers(self, needed: Role) -> bool {
        self >= needed
    }
}

/// A bearer token's resolved claims, as stored in a token-registry Secret:
/// who (`subject`) and what they may do, keyed by a generic resource string
/// (a service's collection/namespace/etc). The literal key `*` is a
/// wildcard grant applied when no more specific entry matches.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenClaims {
    pub subject: String,
    /// `resource` → `Role`. The literal key `*` is a wildcard.
    #[serde(default)]
    pub roles: HashMap<String, Role>,
}

/// Pure: extract `spec.tokensSecret` from a CR's `kubectl get -o json`
/// output (the shared token-registry-Secret CR convention).
pub fn cr_tokens_secret(cr_json: &serde_json::Value) -> Option<String> {
    cr_json["spec"]["tokensSecret"].as_str().map(str::to_string)
}

/// Pick the first registry token whose roles cover `role` for `collection`
/// (falling back to the wildcard `*` grant). Pure — unit-testable without
/// any I/O; deterministic tie-break is not needed since callers name a
/// specific role/collection scope for their own token.
pub fn select_token(
    registry: &HashMap<String, TokenClaims>,
    role: Role,
    collection: Option<&str>,
) -> Option<String> {
    registry.iter().find_map(|(token, claims)| {
        let granted = collection
            .and_then(|c| claims.roles.get(c))
            .or_else(|| claims.roles.get("*"));
        granted
            .is_some_and(|granted| granted.covers(role))
            .then(|| token.clone())
    })
}
