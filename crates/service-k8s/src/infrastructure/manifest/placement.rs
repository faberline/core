//! Failure-domain placement primitives: group-scoped host anti-affinity,
//! zone spread constraints, tolerations, and node selectors.
//!
//! Provides typed helpers for app-defined replica-group labels, host separation,
//! and zone spread intent without requiring apps to construct raw Kubernetes
//! scheduling JSON expressions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const TOPOLOGY_KEY_HOSTNAME: &str = "kubernetes.io/hostname";
pub const TOPOLOGY_KEY_ZONE: &str = "topology.kubernetes.io/zone";

/// Host separation mode across nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostSeparation {
    /// No host anti-affinity expression emitted.
    None,
    /// Hard anti-affinity: `requiredDuringSchedulingIgnoredDuringExecution`.
    Hard,
    /// Soft anti-affinity: `preferredDuringSchedulingIgnoredDuringExecution` with a weight.
    Soft(i32),
}

impl HostSeparation {
    pub const DEFAULT_SOFT_WEIGHT: i32 = 100;

    /// Preferred anti-affinity with the default weight (100).
    pub fn soft() -> Self {
        Self::Soft(Self::DEFAULT_SOFT_WEIGHT)
    }

    /// Preferred anti-affinity with a custom weight.
    pub fn soft_with_weight(weight: i32) -> Self {
        Self::Soft(weight)
    }
}

/// Zone spread intent across failure domains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZoneSpread {
    /// No topology spread constraint emitted.
    None,
    /// Required spread (`whenUnsatisfiable: DoNotSchedule`).
    Required { max_skew: u32, topology_key: String },
    /// Preferred spread (`whenUnsatisfiable: ScheduleAnyway`).
    Preferred { max_skew: u32, topology_key: String },
}

impl ZoneSpread {
    /// Required zone spread across `topology.kubernetes.io/zone`.
    pub fn required(max_skew: u32) -> Self {
        Self::Required {
            max_skew,
            topology_key: TOPOLOGY_KEY_ZONE.to_string(),
        }
    }

    /// Preferred zone spread across `topology.kubernetes.io/zone`.
    pub fn preferred(max_skew: u32) -> Self {
        Self::Preferred {
            max_skew,
            topology_key: TOPOLOGY_KEY_ZONE.to_string(),
        }
    }

    /// Required spread with a custom topology key.
    pub fn required_with_key(max_skew: u32, topology_key: impl Into<String>) -> Self {
        Self::Required {
            max_skew,
            topology_key: topology_key.into(),
        }
    }

    /// Preferred spread with a custom topology key.
    pub fn preferred_with_key(max_skew: u32, topology_key: impl Into<String>) -> Self {
        Self::Preferred {
            max_skew,
            topology_key: topology_key.into(),
        }
    }
}

/// A Kubernetes toleration definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toleration {
    pub key: String,
    pub operator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub effect: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "tolerationSeconds")]
    pub toleration_seconds: Option<u64>,
}

impl Toleration {
    /// Toleration with `operator: "Equal"`.
    pub fn equal(
        key: impl Into<String>,
        value: impl Into<String>,
        effect: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            operator: "Equal".to_string(),
            value: Some(value.into()),
            effect: effect.into(),
            toleration_seconds: None,
        }
    }

    /// Toleration with `operator: "Exists"`.
    pub fn exists(key: impl Into<String>, effect: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            operator: "Exists".to_string(),
            value: None,
            effect: effect.into(),
            toleration_seconds: None,
        }
    }

    /// Construct a toleration with explicit fields.
    pub fn new(
        key: impl Into<String>,
        operator: impl Into<String>,
        value: Option<String>,
        effect: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            operator: operator.into(),
            value,
            effect: effect.into(),
            toleration_seconds: None,
        }
    }

    /// Attach toleration seconds.
    pub fn with_toleration_seconds(mut self, seconds: u64) -> Self {
        self.toleration_seconds = Some(seconds);
        self
    }
}

/// Errors raised by invalid placement inputs.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PlacementError {
    #[error("invalid label key '{0}': must follow Kubernetes syntax (optional prefix <= 253 chars, name <= 63 chars, alphanumeric with '-', '_', '.')")]
    InvalidLabelKey(String),

    #[error("invalid label value '{0}': must follow Kubernetes syntax (non-empty, <= 63 chars, alphanumeric with '-', '_', '.')")]
    InvalidLabelValue(String),

    #[error("empty topology key: topology key must not be empty")]
    EmptyTopologyKey,

    #[error("invalid maxSkew: maxSkew must be greater than 0")]
    InvalidMaxSkew,

    #[error("invalid toleration operator '{0}': must be 'Equal' or 'Exists'")]
    InvalidTolerationOperator(String),

    #[error(
        "invalid toleration effect '{0}': must be 'NoSchedule', 'PreferNoSchedule', or 'NoExecute'"
    )]
    InvalidTolerationEffect(String),

    #[error("unschedulable: replicas ({replicas}) exceeds available host domains ({hosts}) with hard host separation")]
    Unschedulable { replicas: u32, hosts: u32 },
}

/// Validates Kubernetes label key syntax.
pub fn validate_label_key(key: &str) -> Result<(), PlacementError> {
    if key.is_empty() {
        return Err(PlacementError::InvalidLabelKey(key.to_string()));
    }
    let (prefix, name) = if let Some((p, n)) = key.split_once('/') {
        (Some(p), n)
    } else {
        (None, key)
    };

    if let Some(p) = prefix {
        if p.is_empty() || p.len() > 253 {
            return Err(PlacementError::InvalidLabelKey(key.to_string()));
        }
        for label in p.split('.') {
            if label.is_empty() || label.len() > 63 {
                return Err(PlacementError::InvalidLabelKey(key.to_string()));
            }
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(PlacementError::InvalidLabelKey(key.to_string()));
            }
            if !label.starts_with(|c: char| c.is_ascii_alphanumeric())
                || !label.ends_with(|c: char| c.is_ascii_alphanumeric())
            {
                return Err(PlacementError::InvalidLabelKey(key.to_string()));
            }
        }
    }

    validate_label_name(name, key)?;
    Ok(())
}

fn validate_label_name(name: &str, full_key: &str) -> Result<(), PlacementError> {
    if name.is_empty() || name.len() > 63 {
        return Err(PlacementError::InvalidLabelKey(full_key.to_string()));
    }
    if !name.starts_with(|c: char| c.is_ascii_alphanumeric())
        || !name.ends_with(|c: char| c.is_ascii_alphanumeric())
    {
        return Err(PlacementError::InvalidLabelKey(full_key.to_string()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(PlacementError::InvalidLabelKey(full_key.to_string()));
    }
    Ok(())
}

/// Validates Kubernetes label value syntax.
pub fn validate_label_value(val: &str) -> Result<(), PlacementError> {
    if val.is_empty() || val.len() > 63 {
        return Err(PlacementError::InvalidLabelValue(val.to_string()));
    }
    if !val.starts_with(|c: char| c.is_ascii_alphanumeric())
        || !val.ends_with(|c: char| c.is_ascii_alphanumeric())
    {
        return Err(PlacementError::InvalidLabelValue(val.to_string()));
    }
    if !val
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(PlacementError::InvalidLabelValue(val.to_string()));
    }
    Ok(())
}

/// Validates a toleration definition.
pub fn validate_toleration(t: &Toleration) -> Result<(), PlacementError> {
    if t.operator != "Equal" && t.operator != "Exists" {
        return Err(PlacementError::InvalidTolerationOperator(
            t.operator.clone(),
        ));
    }
    if t.effect != "NoSchedule" && t.effect != "PreferNoSchedule" && t.effect != "NoExecute" {
        return Err(PlacementError::InvalidTolerationEffect(t.effect.clone()));
    }
    Ok(())
}

/// Typed planner for failure-domain placement.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementPlan {
    pub group_key: String,
    pub group_value: String,
    pub host_separation: HostSeparation,
    pub zone_spread: ZoneSpread,
    pub node_selectors: BTreeMap<String, String>,
    pub tolerations: Vec<Toleration>,
    pub domain_counts: Option<(u32, u32)>,
}

pub type FailureDomainPlacement = PlacementPlan;

impl PlacementPlan {
    /// Construct a new placement plan for the specified replica-group label.
    ///
    /// Defaults to hard host separation and no zone spread.
    pub fn new(group_key: impl Into<String>, group_value: impl Into<String>) -> Self {
        Self {
            group_key: group_key.into(),
            group_value: group_value.into(),
            host_separation: HostSeparation::Hard,
            zone_spread: ZoneSpread::None,
            node_selectors: BTreeMap::new(),
            tolerations: Vec::new(),
            domain_counts: None,
        }
    }

    /// Set host separation mode.
    pub fn with_host_separation(mut self, mode: HostSeparation) -> Self {
        self.host_separation = mode;
        self
    }

    /// Set zone spread mode.
    pub fn with_zone_spread(mut self, mode: ZoneSpread) -> Self {
        self.zone_spread = mode;
        self
    }

    /// Add a single node selector key/value pair.
    pub fn with_node_selector(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.node_selectors.insert(key.into(), value.into());
        self
    }

    /// Add multiple node selector key/value pairs.
    pub fn with_node_selectors(
        mut self,
        selectors: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in selectors {
            self.node_selectors.insert(k.into(), v.into());
        }
        self
    }

    /// Add a toleration.
    pub fn with_toleration(mut self, toleration: Toleration) -> Self {
        self.tolerations.push(toleration);
        self
    }

    /// Add multiple tolerations.
    pub fn with_tolerations(mut self, tolerations: impl IntoIterator<Item = Toleration>) -> Self {
        self.tolerations.extend(tolerations);
        self
    }

    /// Declare replica count and available host count to validate feasibility.
    pub fn with_domain_counts(mut self, replicas: u32, available_hosts: u32) -> Self {
        self.domain_counts = Some((replicas, available_hosts));
        self
    }

    /// The group label pair as references `(&key, &value)` for templates.
    pub fn group_label(&self) -> (&str, &str) {
        (&self.group_key, &self.group_value)
    }

    /// The group label pair as owned `(key, value)`.
    pub fn label_pair(&self) -> (String, String) {
        (self.group_key.clone(), self.group_value.clone())
    }

    /// Validate all placement parameters.
    pub fn validate(&self) -> Result<(), PlacementError> {
        validate_label_key(&self.group_key)?;
        validate_label_value(&self.group_value)?;

        for (k, v) in &self.node_selectors {
            validate_label_key(k)?;
            validate_label_value(v)?;
        }

        match &self.zone_spread {
            ZoneSpread::None => {}
            ZoneSpread::Required {
                max_skew,
                topology_key,
            }
            | ZoneSpread::Preferred {
                max_skew,
                topology_key,
            } => {
                if topology_key.is_empty() {
                    return Err(PlacementError::EmptyTopologyKey);
                }
                if *max_skew == 0 {
                    return Err(PlacementError::InvalidMaxSkew);
                }
            }
        }

        for t in &self.tolerations {
            validate_toleration(t)?;
        }

        if let Some((replicas, hosts)) = self.domain_counts {
            if self.host_separation == HostSeparation::Hard && replicas > hosts {
                return Err(PlacementError::Unschedulable { replicas, hosts });
            }
        }

        Ok(())
    }

    /// Render optional affinity fragment for `with_affinity` or `plan.affinity`.
    pub fn optional_affinity(&self) -> Result<Option<Value>, PlacementError> {
        self.validate()?;
        match self.host_separation {
            HostSeparation::None => Ok(None),
            HostSeparation::Hard => Ok(Some(json!({
                "podAntiAffinity": {
                    "requiredDuringSchedulingIgnoredDuringExecution": [{
                        "labelSelector": {
                            "matchLabels": {
                                &self.group_key: &self.group_value,
                            }
                        },
                        "topologyKey": TOPOLOGY_KEY_HOSTNAME,
                    }]
                }
            }))),
            HostSeparation::Soft(weight) => Ok(Some(json!({
                "podAntiAffinity": {
                    "preferredDuringSchedulingIgnoredDuringExecution": [{
                        "weight": weight,
                        "podAffinityTerm": {
                            "labelSelector": {
                                "matchLabels": {
                                    &self.group_key: &self.group_value,
                                }
                            },
                            "topologyKey": TOPOLOGY_KEY_HOSTNAME,
                        }
                    }]
                }
            }))),
        }
    }

    /// Render affinity `Value` fragment directly suitable for `with_affinity(...)`.
    ///
    /// Returns empty object `{}` when host separation is None.
    pub fn affinity(&self) -> Result<Value, PlacementError> {
        Ok(self.optional_affinity()?.unwrap_or_else(|| json!({})))
    }

    /// Render tolerations array fragment for `plan.tolerations` or `with_tolerations(...)`.
    pub fn tolerations(&self) -> Result<Vec<Value>, PlacementError> {
        self.validate()?;
        let mut list = Vec::with_capacity(self.tolerations.len());
        for t in &self.tolerations {
            list.push(
                serde_json::to_value(t)
                    .map_err(|_| PlacementError::InvalidTolerationOperator(t.operator.clone()))?,
            );
        }
        Ok(list)
    }

    /// Render topology spread constraints array for `plan.topology_spread_constraints`.
    pub fn topology_spread_constraints(&self) -> Result<Vec<Value>, PlacementError> {
        self.validate()?;
        match &self.zone_spread {
            ZoneSpread::None => Ok(Vec::new()),
            ZoneSpread::Required {
                max_skew,
                topology_key,
            } => Ok(vec![json!({
                "maxSkew": max_skew,
                "topologyKey": topology_key,
                "whenUnsatisfiable": "DoNotSchedule",
                "labelSelector": {
                    "matchLabels": {
                        &self.group_key: &self.group_value,
                    }
                }
            })]),
            ZoneSpread::Preferred {
                max_skew,
                topology_key,
            } => Ok(vec![json!({
                "maxSkew": max_skew,
                "topologyKey": topology_key,
                "whenUnsatisfiable": "ScheduleAnyway",
                "labelSelector": {
                    "matchLabels": {
                        &self.group_key: &self.group_value,
                    }
                }
            })]),
        }
    }

    /// Render node selector `Value` fragment for `plan.node_selector`.
    pub fn node_selector(&self) -> Result<Value, PlacementError> {
        self.validate()?;
        Ok(json!(self.node_selectors))
    }
}

#[cfg(test)]
mod tests;
