//! Kubernetes pod lifecycle and termination budget validation.
//!
//! A workload pod's shutdown sequence must execute within the Kubernetes
//! `terminationGracePeriodSeconds` allocated to it. The budget includes
//! an in-process runtime deadline (`runtime_deadline_seconds`), a trailing
//! SIGKILL reserve (`sigkill_reserve_seconds`), application-declared minimum
//! hook duration (`min_hook_duration_seconds`), an optional preStop drain cost
//! (`prestop_cost_seconds`), and probe timing definitions.

pub use crate::domain::lifecycle::{
    LifecyclePolicy, LifecyclePolicyError, ProbeTiming, TerminationBudget, DRAIN_ENDPOINT_PATH,
    ENV_SERVICE_RUNTIME_DEADLINE_SECONDS, ENV_SERVICE_SIGKILL_RESERVE_SECONDS,
    HEALTH_ENDPOINT_PATH, READY_ENDPOINT_PATH, TERMINATION_BUDGET_CONDITION,
};
