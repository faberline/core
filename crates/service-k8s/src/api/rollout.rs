//! Kubernetes StatefulSet rolling update decision primitives.
//!
//! A pure planner that decides, from observed StatefulSet facts and app-supplied
//! health facts, which single member a StatefulSet `RollingUpdate` may update next.
//! It returns the `spec.updateStrategy.rollingUpdate.partition` value to set, or a
//! hold with a reason.

pub use crate::domain::rollout::{
    plan_rollout, HoldReason, ObservedMember, ObservedRollout, Ordinal, RolloutGate, RolloutStep,
};
