//! Validation for app-declared access grants.

use std::collections::HashSet;

use super::{error::AccessGrantError, grant::AccessGrant};

/// Scan `grant` for any string containing a `*`, returning the path of the
/// first occurrence.
///
/// Implements the scan semantics of `first_wildcard` in the domain layer
/// without importing the infrastructure layer.
fn find_wildcard(grant: &AccessGrant) -> Option<String> {
    if grant.namespace.contains('*') {
        return Some("namespace".to_string());
    }
    if grant.name.contains('*') {
        return Some("name".to_string());
    }
    for (i, rule) in grant.rules.iter().enumerate() {
        for (j, ag) in rule.api_groups.iter().enumerate() {
            if ag.contains('*') {
                return Some(format!("rules/{i}/apiGroups/{j}"));
            }
        }
        for (j, res) in rule.resources.iter().enumerate() {
            if res.contains('*') {
                return Some(format!("rules/{i}/resources/{j}"));
            }
        }
        for (j, rn) in rule.resource_names.iter().enumerate() {
            if rn.contains('*') {
                return Some(format!("rules/{i}/resourceNames/{j}"));
            }
        }
        for (j, v) in rule.verbs.iter().enumerate() {
            if v.contains('*') {
                return Some(format!("rules/{i}/verbs/{j}"));
            }
        }
    }
    for (i, subject) in grant.subjects.iter().enumerate() {
        if subject.namespace.contains('*') {
            return Some(format!("subjects/{i}/namespace"));
        }
        if subject.name.contains('*') {
            return Some(format!("subjects/{i}/name"));
        }
    }
    None
}

/// Validate a slice of [`AccessGrant`]s.
///
/// An empty slice passes validation and represents an explicit deny-all policy.
pub fn validate_policy(grants: &[AccessGrant]) -> Result<(), AccessGrantError> {
    if grants.is_empty() {
        return Ok(());
    }

    let mut seen = HashSet::new();

    for grant in grants {
        // Two grants with the same (namespace, name)
        if !seen.insert((grant.namespace.clone(), grant.name.clone())) {
            return Err(AccessGrantError::DuplicateGrant {
                grant: grant.name.clone(),
                namespace: grant.namespace.clone(),
            });
        }

        // Wildcard check
        if let Some(path) = find_wildcard(grant) {
            return Err(AccessGrantError::Wildcard {
                grant: grant.name.clone(),
                path,
            });
        }

        // A grant with no rules or no subjects
        if grant.rules.is_empty() || grant.subjects.is_empty() {
            return Err(AccessGrantError::EmptyGrant {
                grant: grant.name.clone(),
            });
        }

        // A subject with an empty namespace or name
        for subject in &grant.subjects {
            if subject.namespace.is_empty() || subject.name.is_empty() {
                return Err(AccessGrantError::InvalidSubject {
                    grant: grant.name.clone(),
                });
            }
        }

        // Rules validation
        for rule in &grant.rules {
            // A rule with no verbs, resources, or api_groups
            if rule.api_groups.is_empty() || rule.resources.is_empty() || rule.verbs.is_empty() {
                return Err(AccessGrantError::EmptyRuleFields {
                    grant: grant.name.clone(),
                });
            }

            // A rule with empty resource_names
            if rule.resource_names.is_empty() {
                return Err(AccessGrantError::EmptyResourceNames {
                    grant: grant.name.clone(),
                });
            }
        }
    }

    Ok(())
}
