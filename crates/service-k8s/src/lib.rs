//! `service-k8s` — the ecosystem's shared Kubernetes operator scaffold.
//!
//! Every axiom service that ships a CRD reconciles the same way: a controller
//! that watches the CR cluster-wide, server-side-applies the rendered child
//! objects, and writes back a status — gated by a leader-election Lease so
//! `replicas > 1` is safe. The watch can be limited to a list of namespaces
//! with `run_in_namespaces`. This crate centralizes that loop + the lease + a
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
pub use api::fleet::{fleet_crd_schema_hint, FleetInstance, FleetRollout, FleetSpec, FleetStatus};
#[cfg(feature = "controller")]
pub use api::lease;
pub use api::lifecycle;
#[cfg(feature = "controller")]
pub use api::llm;
#[cfg(feature = "controller")]
pub use api::metrics;
pub use api::profile;
pub use api::render;
#[cfg(feature = "controller")]
pub use api::resize;
pub use api::rollout;
pub use api::service;
pub use api::stateful;
pub use api::trust_bundle;

#[cfg(feature = "controller")]
pub use app::operator::{run, run_in_namespaces};
#[cfg(feature = "certificate")]
pub use application::certificate::reconcile::Reconciler;
#[cfg(feature = "controller")]
pub use application::operator::managed_service::{
    ClusterScopedChild, ManagedService, ReadinessTarget, ReadyFacts,
};
pub use application::reshard::{
    run_reshard, ReshardCluster, ReshardConfig, ReshardError, ReshardOutcome,
};
#[cfg(feature = "controller")]
pub use domain::access::{
    access_policy_condition, validate_policy, AccessGrant, AccessGrantError, AccessRule,
    AccessSubject, GrantOutcome, ACCESS_POLICY_READY_CONDITION,
};
pub use domain::backup_job::{
    backup_job_name, plan_backup_job, BackupJobDecision, BackupJobError, BackupJobPhase,
    BackupJobPolicy, ObservedBackupJob,
};
pub use domain::backup_status::{
    backup_staleness_seconds, last_backup_succeeded, next_backup_status, BackupAttempt,
    BackupJobOutcome, BackupManifestRef, BackupStatus, LatestBackup,
};
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
#[cfg(feature = "controller")]
pub use domain::fleet;
#[cfg(feature = "controller")]
pub use domain::fleet::{
    aggregate_fleet_condition, canonicalize_json, desired_hash, plan_fleet, plan_rollout_order,
    reject_duplicate_targets, AdoptionPolicy, ChildKey, DesiredChild, FleetAction, FleetAggregate,
    FleetEntry, FleetEntryPhase, FleetEntryStatus, FleetPlan, FleetPlanError, FleetTarget,
    ObservedChild, Ownership, PrunePolicy, RolloutState,
};
#[cfg(feature = "controller")]
pub use domain::leadership::Election;
pub use domain::lifecycle::{
    LifecyclePolicy, LifecyclePolicyError, ProbeTiming, TerminationBudget,
};
pub use domain::membership::{
    plan_membership_step, DesiredMembership, MemberRole, MemberState, MembershipStep,
    ObservedMembership, ReadScalingMode, ShardMembership,
};
pub use domain::replica_status::{
    flat_status_patch, replica_phase, replicas_message, ReplicaPhase,
};
pub use domain::reshard::{
    decide_split, next_action, rollback_plan, BucketData, BucketDataError, CopyPass, CopyReport,
    ObservedReshard, ReshardAction, ReshardState, ReshardStep, ReshardWait, RollbackAction,
    SplitDecision, StepOutcome,
};
pub use domain::restore_job::{
    next_restore_status, plan_restore_job, restore_finalizer_removable, restore_gates_readiness,
    restore_job_name, ObservedRestoreJob, RestoreJobDecision, RestoreJobError, RestoreJobOutcome,
    RestoreJobPhase, RestoreJobPolicy, RestorePhase, RestoreRequest, RestoreStatus,
    RESTORE_FINALIZER,
};
pub use domain::retention::{DataRetention, RetentionPlan, INSTANCE_UID_LABEL};
pub use domain::shard_map::{
    rebuild_shard_map, PodShardReport, ShardMap, ShardMapError, SHARD_MAP_KEY,
};
pub use domain::shard_migration::{
    plan_shard_migration, ShardMigrationError, ShardMigrationPlan, StatefulSetShape,
};
pub use domain::spec_defaults::{
    DEFAULT_AUTH_MODE, DEFAULT_BODY_LIMIT_BYTES, DEFAULT_GRACE_SECS, DEFAULT_STORAGE_SIZE,
};
#[cfg(feature = "certificate")]
pub use infrastructure::certificate::csr::RcgenCsrGenerator;
#[cfg(feature = "certificate")]
pub use infrastructure::certificate::leaf_parser::X509LeafParser;
#[cfg(feature = "controller")]
pub use infrastructure::manifest::render_access_grants;
#[cfg(feature = "controller")]
pub use interfaces::cluster_spec::{ClusterSpec, ResourceSpec};
#[cfg(feature = "controller")]
pub use interfaces::metrics::ControllerMetrics;
#[cfg(feature = "controller")]
pub use interfaces::operator::Error;
#[cfg(feature = "controller")]
pub use interfaces::operator::WatchScope;

#[cfg(feature = "controller")]
pub use application::access_reconcile::{reconcile_access, AccessReport};
#[cfg(feature = "controller")]
pub use application::membership::{run_membership_step, run_membership_step_as_leader};
#[cfg(feature = "controller")]
pub use domain::access::{AccessObjectKind, AccessOutcome, AccessStore};
#[cfg(feature = "controller")]
pub use domain::membership_port::{
    MembershipError, MembershipExecutor, Progress, RaftAdminClient, StepResult,
};
#[cfg(feature = "controller")]
pub use infrastructure::access_kubernetes_store::{KubernetesAccessStore, ACCESS_FIELD_MANAGER};
#[cfg(feature = "controller")]
pub use infrastructure::access_store::InMemoryAccessStore;
#[cfg(feature = "controller")]
pub use infrastructure::membership_executor::{KubeMembershipExecutor, MembershipNaming};
#[cfg(feature = "raft-admin-http")]
pub use infrastructure::raft_admin_http::{HttpRaftAdminClient, RaftAdminTopology};
