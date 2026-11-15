//! The `ServiceProfile` contract an app declares, and its pure validation (ADR 0002 D4).
//!
//! No Kubernetes object is rendered here. The alert rules are plain data that
//! core renders later.

use std::collections::BTreeSet;

/// How a service splits its data across shards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Partitioning {
    /// One shard.
    Single,
    /// Several buckets. Not supported by the first release.
    Bucketed,
}

/// Whether read traffic can be served by extra learner replicas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadScaling {
    /// Reads go to the voters only.
    None,
    /// Reads can be served by learner replicas.
    Learners,
}

/// The capabilities a service declares. Both sets are closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// How the data is partitioned.
    pub partitioning: Partitioning,
    /// Whether read traffic can scale out.
    pub read_scaling: ReadScaling,
}

/// One alert rule as plain data. Core renders it later.
///
/// This is not the borrowed `AlertRule<'a>` of the manifest renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertRule {
    /// Alert name.
    pub name: String,
    /// PromQL expression.
    pub expression: String,
    /// `for` duration, for example `"5m"`.
    pub for_duration: String,
    /// `severity` label value.
    pub severity: String,
    /// `summary` annotation.
    pub summary: String,
}

/// What an app declares about its service.
pub trait ServiceProfile {
    /// Kind name of the service. Must not be empty.
    const KIND: &'static str;
    /// Metric and environment prefix: upper-case ASCII letters, digits and `_`.
    const PREFIX: &'static str;
    /// The app's configuration type. No bound.
    type Config;

    /// The capabilities the service declares.
    fn capabilities() -> Capabilities;

    /// The alert rules the service declares.
    fn alerts() -> Vec<AlertRule>;
}

/// Why a profile is refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// `KIND` is empty.
    #[error("profile KIND is empty")]
    EmptyKind,
    /// `PREFIX` is empty or has a character outside upper-case ASCII letters, digits and `_`.
    #[error(
        "profile PREFIX `{prefix}` must be upper-case ASCII letters, digits and `_`, and not empty"
    )]
    InvalidPrefix {
        /// The refused prefix.
        prefix: String,
    },
    /// Two alert rules have the same name.
    #[error("duplicate alert name `{name}`")]
    DuplicateAlert {
        /// The repeated name.
        name: String,
    },
    /// A declared capability is outside the supported set.
    #[error("capability `{capability}` is not supported in the first release")]
    UnsupportedCapability {
        /// The refused capability, for example `partitioning=bucketed`.
        capability: &'static str,
    },
}

/// Checks a profile. Returns the first failure in this order: empty `KIND`,
/// invalid `PREFIX`, a repeated alert name, then an unsupported capability.
pub fn validate_profile<P: ServiceProfile>() -> Result<(), ProfileError> {
    if P::KIND.is_empty() {
        return Err(ProfileError::EmptyKind);
    }

    let prefix_ok = !P::PREFIX.is_empty()
        && P::PREFIX
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !prefix_ok {
        return Err(ProfileError::InvalidPrefix {
            prefix: P::PREFIX.to_string(),
        });
    }

    let alerts = P::alerts();
    let mut seen = BTreeSet::new();
    for alert in &alerts {
        if !seen.insert(alert.name.as_str()) {
            return Err(ProfileError::DuplicateAlert {
                name: alert.name.clone(),
            });
        }
    }

    if P::capabilities().partitioning == Partitioning::Bucketed {
        return Err(ProfileError::UnsupportedCapability {
            capability: "partitioning=bucketed",
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests;
