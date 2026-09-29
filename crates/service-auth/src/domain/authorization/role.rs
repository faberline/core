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
    pub subject: String,
    /// `resource` → `Role`. The literal key `*` is a wildcard.
    #[serde(default)]
    pub roles: HashMap<String, Role>,
}
