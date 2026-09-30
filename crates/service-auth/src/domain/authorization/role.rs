use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Role hierarchy: `Admin` ⊇ `Write` ⊇ `Read`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

/// A bearer token's resolved claims: who (`subject`) and what they may do,
/// keyed by a generic resource string. `*` is a wildcard grant applied when
/// no more specific entry matches.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenClaims {
    subject: String,
    #[serde(default)]
    roles: HashMap<String, Role>,
}

impl TokenClaims {
    /// Claims for `subject`, with `roles` keyed by resource (`*` is the
    /// wildcard).
    pub fn new(subject: impl Into<String>, roles: HashMap<String, Role>) -> Self {
        Self {
            subject: subject.into(),
            roles,
        }
    }

    /// Who the token belongs to.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// `resource` → `Role`. The literal key `*` is a wildcard.
    pub fn roles(&self) -> &HashMap<String, Role> {
        &self.roles
    }
}
