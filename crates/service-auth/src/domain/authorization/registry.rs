use std::collections::HashMap;

use anyhow::{bail, Context as _, Result};

use super::role::TokenClaims;

/// Section name for bearer-secret-keyed entries in a namespaced registry.
const TOKENS_SECTION: &str = "tokens";
/// Section name for identity-keyed entries in a namespaced registry.
const IDENTITIES_SECTION: &str = "identities";

/// A credential registry with two **disjoint** key namespaces (#2678).
///
/// - `tokens` is keyed by the bearer secret itself. The key *is* the
///   credential, which is why a file containing one has to be stored as a
///   secret.
/// - `identities` is keyed by a principal an identity provider has already
///   verified — a Google email, resolved by [`crate::gcp::GoogleVerifier`].
///   An email is public, so this half is ordinary configuration.
///
/// Keeping them apart is a security property, not tidiness: a shared map would
/// let a bearer secret that happens to be shaped like an email match an
/// identity entry and silently acquire its grants.
///
/// ## Document shapes
///
/// Namespaced (the shape that can carry identities):
///
/// ```json
/// { "tokens":     { "<secret>": { "subject": "svc", "roles": { "*": "read" } } },
///   "identities": { "a@b.com":  { "subject": "a",   "roles": { "*": "read" } } } }
/// ```
///
/// Flat (every existing service's file — every key is a bearer secret):
///
/// ```json
/// { "<secret>": { "subject": "svc", "roles": { "*": "read" } } }
/// ```
///
/// A document is read as namespaced when every top-level key is `tokens` or
/// `identities` **and** no top-level value is itself a claims object. The
/// second clause is what keeps a flat registry whose single secret is literally
/// spelled `tokens` from being misread as a section.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// Keyed by the bearer secret presented in `Authorization: Bearer …`.
    pub tokens: HashMap<String, TokenClaims>,
    /// Keyed by an identity an external provider verified (a Google email).
    pub identities: HashMap<String, TokenClaims>,
}

impl Registry {
    /// A bearer-only registry — the shape every service had before #2678.
    pub fn from_tokens(tokens: HashMap<String, TokenClaims>) -> Self {
        Self {
            tokens,
            identities: HashMap::new(),
        }
    }

    /// Total entries across both namespaces.
    pub fn len(&self) -> usize {
        self.tokens.len() + self.identities.len()
    }

    /// Whether the registry can authenticate nobody at all.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty() && self.identities.is_empty()
    }

    /// Parse either document shape. See the type docs for the discriminator.
    pub fn parse(json: &str) -> Result<Self> {
        let doc: serde_json::Value =
            serde_json::from_str(json).context("credential registry must be JSON")?;
        let namespaced = {
            let map = doc
                .as_object()
                .context("credential registry must be a JSON object")?;
            map.keys()
                .all(|key| key == TOKENS_SECTION || key == IDENTITIES_SECTION)
                && !map.values().any(|value| value.get("subject").is_some())
        };
        if !namespaced {
            return Ok(Self::from_tokens(serde_json::from_value(doc).context(
                "credential registry must map each bearer secret to its claims",
            )?));
        }
        let mut map = match doc {
            serde_json::Value::Object(map) => map,
            _ => unreachable!("checked as_object above"),
        };
        let mut section = |name: &str, what: &str| -> Result<HashMap<String, TokenClaims>> {
            match map.remove(name) {
                Some(value) => serde_json::from_value(value)
                    .with_context(|| format!("credential registry `{name}` must map {what}")),
                None => Ok(HashMap::new()),
            }
        };
        Ok(Self {
            tokens: section(TOKENS_SECTION, "each bearer secret to its claims")?,
            identities: section(IDENTITIES_SECTION, "each verified identity to its claims")?,
        })
    }

    /// Union another registry into this one, per namespace.
    ///
    /// The two namespaces have different confidentiality classes, so a
    /// deployment may well project them from different places — an
    /// `identities` map from a ConfigMap and a `tokens` map from a Secret
    /// (#2764). Merging is how those reunite into the one registry a request
    /// is resolved against.
    ///
    /// A key present in both inputs' *same* namespace is an error rather than
    /// a last-writer-wins overwrite: two sources disagreeing about one
    /// principal's grants leaves nobody able to say which grants are actually
    /// being served, which is the failure mode the split was meant to avoid.
    /// The same key appearing in *different* namespaces is not a collision —
    /// they are disjoint by construction (#2678, R1).
    pub fn try_merge(&mut self, other: Registry) -> Result<()> {
        merge_namespace(&mut self.tokens, other.tokens, TOKENS_SECTION)?;
        merge_namespace(&mut self.identities, other.identities, IDENTITIES_SECTION)
    }

    /// The first entry whose `subject` is one a service has reserved for its
    /// own use, as `(namespace, key, subject)`.
    ///
    /// A reserved subject is one the service itself presents — lumen's control
    /// plane names itself in every admin call it makes (#2679). A tenant
    /// registry that claims the same subject would make the operator's calls
    /// and a tenant's calls indistinguishable in audit output, so a registry
    /// carrying one is rejected rather than merged.
    pub fn reserved_subject_violation(
        &self,
        reserved: &[String],
    ) -> Option<(&'static str, String, String)> {
        let hit = |section: &'static str, entries: &HashMap<String, TokenClaims>| {
            entries
                .iter()
                .filter(|(_, claims)| reserved.iter().any(|r| r == &claims.subject))
                .map(|(key, claims)| (section, key.clone(), claims.subject.clone()))
                .min()
        };
        hit(TOKENS_SECTION, &self.tokens).or_else(|| hit(IDENTITIES_SECTION, &self.identities))
    }
}

fn merge_namespace(
    into: &mut HashMap<String, TokenClaims>,
    from: HashMap<String, TokenClaims>,
    section: &str,
) -> Result<()> {
    for (key, claims) in from {
        if let Some(previous) = into.get(&key) {
            // An `identities` key is a public email, and naming it is the
            // difference between a fixable message and a scavenger hunt. A
            // `tokens` key IS the bearer secret, so it is named by the subject
            // it grants instead — never by the key.
            let culprit = if section == IDENTITIES_SECTION {
                format!("`{key}`")
            } else {
                format!("the entry granting `{}`", previous.subject)
            };
            bail!(
                "credential registry sources disagree: `{section}` defines {culprit} more than \
                 once, so there is no way to say which grants are being served"
            );
        }
        into.insert(key, claims);
    }
    Ok(())
}
