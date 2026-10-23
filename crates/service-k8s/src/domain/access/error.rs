//! Typed errors for access grant validation.

/// Validation error for an [`super::AccessGrant`].
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
    /// Return the name of the offending grant.
    pub fn grant(&self) -> &str {
        match self {
            Self::Wildcard { grant, .. } => grant,
            Self::EmptyResourceNames { grant } => grant,
            Self::EmptyRuleFields { grant } => grant,
            Self::EmptyGrant { grant } => grant,
            Self::DuplicateGrant { grant, .. } => grant,
            Self::InvalidSubject { grant } => grant,
        }
    }
}
