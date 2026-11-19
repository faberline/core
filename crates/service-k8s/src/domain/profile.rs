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

/// The Kubernetes kind of an extra workload. The same four kinds that
/// `WorkloadPlan` (`infrastructure/manifest/workload_plan.rs`) can add:
/// `StatefulSetPlan`, `DeploymentPlan`, `DaemonSetPlan`, `CronJobPlan`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkloadKind {
    /// A StatefulSet.
    StatefulSet,
    /// A Deployment.
    Deployment,
    /// A DaemonSet.
    DaemonSet,
    /// A CronJob.
    CronJob,
}

/// One extra workload a profile declares next to its one StatefulSet.
///
/// `name` is a suffix: the object of the workload is named `<instance>-<name>`
/// (#528 renders it). It is a DNS label part: 1 to 30 characters, lower-case
/// ASCII letters, digits and `-`, starting and ending with a letter or digit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtraWorkload {
    /// The suffix of the workload name, for example `collector`.
    pub name: String,
    /// The Kubernetes kind of the workload.
    pub kind: WorkloadKind,
}

/// Workload names that a core child of the instance already uses.
///
/// Core renders `<instance>-config` (ConfigMap), `<instance>-headless`
/// (headless Service) and `<instance>-alerts` (PrometheusRule), and gives
/// every child the component label `server` (`INSTANCE_COMPONENT` in
/// `infrastructure/manifest/instance_render.rs`). An extra workload must not
/// use any of these four names.
pub const RESERVED_WORKLOAD_NAMES: [&str; 4] = ["alerts", "config", "headless", "server"];

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

    /// The extra workloads the service declares. The default is none.
    fn extra_workloads() -> Vec<ExtraWorkload> {
        Vec::new()
    }
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
    /// An extra workload name is not a valid DNS label part.
    #[error("extra workload name `{name}` must be 1 to 30 lower-case ASCII letters, digits or `-`, starting and ending with a letter or digit")]
    InvalidWorkloadName {
        /// The refused name.
        name: String,
    },
    /// An extra workload name collides with a core child of the instance.
    #[error("extra workload name `{name}` collides with a core child of the instance")]
    WorkloadNameCollision {
        /// The refused name.
        name: String,
    },
    /// Two extra workloads have the same name.
    #[error("duplicate extra workload name `{name}`")]
    DuplicateWorkload {
        /// The repeated name.
        name: String,
    },
}

/// True when `name` is 1 to 30 characters of lower-case ASCII letters, digits
/// and `-`, starting and ending with a letter or digit.
fn is_valid_workload_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let is_edge = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    match (bytes.first(), bytes.last()) {
        (Some(&first), Some(&last)) => {
            bytes.len() <= 30
                && is_edge(first)
                && is_edge(last)
                && bytes.iter().all(|&c| is_edge(c) || c == b'-')
        }
        _ => false,
    }
}

/// Checks a profile. Returns the first failure in this order: empty `KIND`,
/// invalid `PREFIX`, a repeated alert name, an unsupported capability, then the
/// extra workloads in declaration order: an invalid name, a core-child
/// collision, a repeated name.
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

    let mut seen_workloads = BTreeSet::new();
    for workload in P::extra_workloads() {
        if !is_valid_workload_name(&workload.name) {
            return Err(ProfileError::InvalidWorkloadName {
                name: workload.name,
            });
        }
        if RESERVED_WORKLOAD_NAMES.contains(&workload.name.as_str()) {
            return Err(ProfileError::WorkloadNameCollision {
                name: workload.name,
            });
        }
        if !seen_workloads.insert(workload.name.clone()) {
            return Err(ProfileError::DuplicateWorkload {
                name: workload.name,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
