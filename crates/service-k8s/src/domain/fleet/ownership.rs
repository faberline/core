//! Ownership classification for observed Fleet children.

use serde::{Deserialize, Serialize};

/// Ownership classification of an observed child object.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Ownership {
    Owned,
    Unowned,
    Foreign { owner: String },
}

impl Ownership {
    /// Derive ownership from a fleet-owner label value and a server-side-apply manager name.
    ///
    /// - If `fleet_owner_label` matches `expected_owner`, the object is [`Ownership::Owned`].
    /// - If `fleet_owner_label` is present with a different owner, the object is [`Ownership::Foreign`].
    /// - If `fleet_owner_label` is absent:
    ///   - If `ssa_manager` matches `expected_manager`, the object is [`Ownership::Owned`].
    ///   - If `ssa_manager` is present with another manager, the object is [`Ownership::Foreign`].
    ///   - If neither is present, the object is [`Ownership::Unowned`].
    pub fn derive(
        fleet_owner_label: Option<&str>,
        ssa_manager: Option<&str>,
        expected_owner: &str,
        expected_manager: &str,
    ) -> Self {
        match fleet_owner_label {
            Some(owner) if owner == expected_owner => {
                if let Some(manager) = ssa_manager {
                    if manager != expected_manager {
                        return Self::Foreign {
                            owner: format!("{owner} (manager: {manager})"),
                        };
                    }
                }
                Self::Owned
            }
            Some(foreign_owner) => Self::Foreign {
                owner: foreign_owner.to_string(),
            },
            None => match ssa_manager {
                Some(manager) if manager == expected_manager => Self::Owned,
                Some(foreign_manager) => Self::Foreign {
                    owner: foreign_manager.to_string(),
                },
                None => Self::Unowned,
            },
        }
    }

    /// Derive ownership from an owner label value against an expected owner.
    pub fn from_owner(fleet_owner_label: Option<&str>, expected_owner: &str) -> Self {
        match fleet_owner_label {
            Some(owner) if owner == expected_owner => Self::Owned,
            Some(owner) => Self::Foreign {
                owner: owner.to_string(),
            },
            None => Self::Unowned,
        }
    }
}
