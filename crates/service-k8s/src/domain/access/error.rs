//! Why a policy is refused. Each variant names the grant at fault.

/// A reason an access policy is refused by [`validate_policy`](super::validate_policy).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AccessGrantError {
    #[error("grant '{grant}' contains wildcard at '{path}'")]
    Wildcard { grant: String, path: String },
    #[error("grant '{grant}' has rule with empty resource_names")]
    EmptyResourceNames { grant: String },
    #[error("grant '{grant}' has rule with no verbs, resources, or api_groups")]
    EmptyRuleFields { grant: String },
    #[error("grant '{grant}' has no rules or no subjects")]
    EmptyGrant { grant: String },
    #[error("duplicate grant '{grant}' in namespace '{namespace}'")]
    DuplicateGrant { grant: String, namespace: String },
    #[error("grant '{grant}' has subject with empty namespace or name")]
    InvalidSubject { grant: String },
}

impl AccessGrantError {
    /// The name of the grant this error is about.
    pub fn grant(&self) -> &str {
        match self {
            Self::Wildcard { grant, .. }
            | Self::EmptyResourceNames { grant }
            | Self::EmptyRuleFields { grant }
            | Self::EmptyGrant { grant }
            | Self::DuplicateGrant { grant, .. }
            | Self::InvalidSubject { grant } => grant,
        }
    }
}
