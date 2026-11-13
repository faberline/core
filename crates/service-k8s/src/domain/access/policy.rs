//! Validation of a whole access policy before anything is rendered or applied.

use std::collections::HashSet;

use super::{AccessGrant, AccessGrantError};

/// Check every grant of a policy, in input order; the first failure wins.
///
/// An empty slice is valid: it is an explicit deny-all, not an error.
pub fn validate_policy(grants: &[AccessGrant]) -> Result<(), AccessGrantError> {
    let mut seen: HashSet<(&str, &str)> = HashSet::new();
    for grant in grants {
        let grant_name = grant.name.clone();
        if !seen.insert((grant.namespace.as_str(), grant.name.as_str())) {
            return Err(AccessGrantError::DuplicateGrant {
                grant: grant_name,
                namespace: grant.namespace.clone(),
            });
        }
        if let Some(path) = find_wildcard(grant) {
            return Err(AccessGrantError::Wildcard {
                grant: grant_name,
                path,
            });
        }
        if grant.rules.is_empty() || grant.subjects.is_empty() {
            return Err(AccessGrantError::EmptyGrant { grant: grant_name });
        }
        if grant
            .subjects
            .iter()
            .any(|s| s.namespace.is_empty() || s.name.is_empty())
        {
            return Err(AccessGrantError::InvalidSubject { grant: grant_name });
        }
        for rule in &grant.rules {
            if rule.api_groups.is_empty() || rule.resources.is_empty() || rule.verbs.is_empty() {
                return Err(AccessGrantError::EmptyRuleFields { grant: grant_name });
            }
            if rule.resource_names.is_empty() {
                return Err(AccessGrantError::EmptyResourceNames { grant: grant_name });
            }
        }
    }
    Ok(())
}

/// The path of the first string in `grant` that contains `*`, in the order
/// namespace, name, then each rule's apiGroups, resources, resourceNames and
/// verbs, then each subject's namespace and name.
fn find_wildcard(grant: &AccessGrant) -> Option<String> {
    let has_star = |s: &str| s.contains('*');
    if has_star(&grant.namespace) {
        return Some("namespace".into());
    }
    if has_star(&grant.name) {
        return Some("name".into());
    }
    for (i, rule) in grant.rules.iter().enumerate() {
        let fields: [(&str, &[String]); 4] = [
            ("apiGroups", &rule.api_groups),
            ("resources", &rule.resources),
            ("resourceNames", &rule.resource_names),
            ("verbs", &rule.verbs),
        ];
        for (field, values) in fields {
            if let Some(j) = values.iter().position(|v| has_star(v)) {
                return Some(format!("rules/{i}/{field}/{j}"));
            }
        }
    }
    for (i, subject) in grant.subjects.iter().enumerate() {
        if has_star(&subject.namespace) {
            return Some(format!("subjects/{i}/namespace"));
        }
        if has_star(&subject.name) {
            return Some(format!("subjects/{i}/name"));
        }
    }
    None
}
