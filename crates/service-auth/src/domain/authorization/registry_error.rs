/// Why a credential registry document could not be read, or why two
/// registries could not be merged.
///
/// No variant carries a bearer secret: a `tokens` key *is* the secret, so a
/// duplicate token is named by the subject it grants, never by its key.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// The document is not JSON.
    #[error("credential registry must be JSON")]
    InvalidJson(#[source] serde_json::Error),
    /// The document is JSON, but not an object.
    #[error("credential registry must be a JSON object")]
    NotAnObject,
    /// A flat document whose values are not claims.
    #[error("credential registry must map each bearer secret to its claims")]
    InvalidFlatTokens(#[source] serde_json::Error),
    /// A namespaced document whose `tokens` section is not claims.
    #[error("credential registry `tokens` must map each bearer secret to its claims")]
    InvalidTokens(#[source] serde_json::Error),
    /// A namespaced document whose `identities` section is not claims.
    #[error("credential registry `identities` must map each verified identity to its claims")]
    InvalidIdentities(#[source] serde_json::Error),
    /// Two merged sources both define one bearer secret.
    #[error(
        "credential registry sources disagree: `tokens` defines the entry granting `{subject}` \
         more than once, so there is no way to say which grants are being served"
    )]
    DuplicateToken {
        /// The subject the first definition grants.
        subject: String,
    },
    /// Two merged sources both define one verified identity.
    #[error(
        "credential registry sources disagree: `identities` defines `{identity}` more than \
         once, so there is no way to say which grants are being served"
    )]
    DuplicateIdentity {
        /// The identity (a public email) defined twice.
        identity: String,
    },
}
