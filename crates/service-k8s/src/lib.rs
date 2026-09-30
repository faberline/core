//! `service-k8s` — the ecosystem's shared Kubernetes operator scaffold.
//!
//! Every axiom service that ships a CRD reconciles the same way: a controller
//! that watches the CR cluster-wide, server-side-applies the rendered child
//! objects, and writes back a status — gated by a leader-election Lease so
//! `replicas > 1` is safe. This crate centralizes that loop + the lease + a
//! render toolkit for the common sharded-HA objects and maintenance CronJobs,
//! so a service supplies only a [`ManagedService`] (its CRD type +
//! `render`/`status_patch`/readiness) and its service-specific rendering.
//!
//! See `CONTRIBUTING.md` "Service archetype" — this is the deploy-layer member of
//! the shared service kit (`raft-core` + `raft-runtime` + `transport-h2c` + `service-http` +
//! `service-backup` + `cli-std` + this).

mod api;
mod app;
mod application;
mod domain;
mod infrastructure;
mod interfaces;

#[cfg(feature = "certificate")]
pub use api::certificate;
#[cfg(feature = "controller")]
pub use api::controller;
pub use api::crd;
#[cfg(feature = "controller")]
pub use api::lease;
pub use api::lifecycle;
#[cfg(feature = "controller")]
pub use api::llm;
#[cfg(feature = "controller")]
pub use api::metrics;
pub use api::render;
#[cfg(feature = "controller")]
pub use api::resize;
pub use api::service;
pub use api::stateful;

#[cfg(feature = "certificate")]
pub use application::certificate::reconcile::Reconciler;
pub use domain::capacity::{
    plan_replica_layer, plan_shard_split, ObservedShardUsage, ObservedUtilization,
    ReplicaLayerError, ReplicaLayerPlan, ReplicaLayerPolicy, ShardSplitError, ShardSplitPlan,
    ShardSplitPolicy, DEFAULT_CPU_REQUEST, DEFAULT_MEMORY_REQUEST,
    DEFAULT_SHARD_SPLIT_THRESHOLD_BYTES,
};
#[cfg(feature = "certificate")]
pub use domain::certificate::issuer::{Issuer, IssuerId, KeyAndCsrGenerator};
#[cfg(feature = "certificate")]
pub use domain::certificate::profile::{CertificateProfile, InstanceScope, Purpose};
#[cfg(feature = "certificate")]
pub use domain::certificate::secret_layout::LeafParser;
#[cfg(feature = "certificate")]
pub use domain::certificate::status::CertificateFacts;
#[cfg(feature = "controller")]
pub use domain::condition::{Condition, ConditionFact, ConditionStatus};
pub use domain::lifecycle::{
    LifecyclePolicy, LifecyclePolicyError, ProbeTiming, TerminationBudget,
};
#[cfg(feature = "certificate")]
pub use infrastructure::certificate::csr::RcgenCsrGenerator;
#[cfg(feature = "certificate")]
pub use infrastructure::certificate::leaf_parser::X509LeafParser;
#[cfg(feature = "controller")]
pub use infrastructure::lease::Election;
#[cfg(feature = "controller")]
pub use interfaces::cluster_spec::{ClusterSpec, ResourceSpec};
#[cfg(feature = "controller")]
pub use interfaces::metrics::ControllerMetrics;
#[cfg(feature = "controller")]
pub use interfaces::operator::managed_service::{
    ClusterScopedChild, ManagedService, ReadinessTarget, ReadyFacts,
};
#[cfg(feature = "controller")]
pub use interfaces::operator::{run, Error};
