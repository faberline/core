//! Per-entry status of a Fleet rollout and its aggregate condition.

use serde::{Deserialize, Serialize};

use super::FleetTarget;

/// Phase of one Fleet entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetEntryPhase {
    /// The entry waits to be applied.
    Pending,
    /// The entry is being applied.
    Applying,
    /// The entry is applied and ready.
    Ready,
    /// The entry failed to apply.
    Failed,
    /// The target cannot be reached.
    Unreachable,
}

impl std::fmt::Display for FleetEntryPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Pending => "Pending",
            Self::Applying => "Applying",
            Self::Ready => "Ready",
            Self::Failed => "Failed",
            Self::Unreachable => "Unreachable",
        };
        f.write_str(name)
    }
}

/// Status of one Fleet entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetEntryStatus {
    /// Target of the entry.
    pub target: FleetTarget,
    /// Current phase.
    pub phase: FleetEntryPhase,
    /// Hash of the spec that was observed, when known.
    pub observed_hash: Option<String>,
    /// Detail message, when any.
    pub message: Option<String>,
}

/// Aggregate condition of a Fleet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetAggregate {
    /// True when every entry is ready.
    pub ready: bool,
    /// Human-readable summary.
    pub message: String,
}

/// Aggregate the entry statuses into one condition.
///
/// The message names the first `Failed` or `Unreachable` entry. When there is none, it names
/// the first entry that is not `Ready`.
pub fn aggregate_fleet_condition(entries: &[FleetEntryStatus]) -> FleetAggregate {
    if entries.is_empty() {
        return FleetAggregate {
            ready: true,
            message: "no entries".to_string(),
        };
    }
    if entries.iter().all(|e| e.phase == FleetEntryPhase::Ready) {
        return FleetAggregate {
            ready: true,
            message: "all entries ready".to_string(),
        };
    }
    let first = entries
        .iter()
        .find(|e| {
            matches!(
                e.phase,
                FleetEntryPhase::Failed | FleetEntryPhase::Unreachable
            )
        })
        .or_else(|| entries.iter().find(|e| e.phase != FleetEntryPhase::Ready))
        .expect("a not-ready entry exists");
    FleetAggregate {
        ready: false,
        message: format!(
            "{} entry {}/{}",
            first.phase,
            first.target.cluster_name(),
            first.target.namespace
        ),
    }
}
