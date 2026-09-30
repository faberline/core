//! Stateful-service capacity primitives shared by every operator adopter.
//!
//! A data workload scales in whole replica layers: with `N` shards, changing
//! replicas-per-shard by one changes the StatefulSet by exactly `N` pods. A
//! vanilla HPA targets total pods and can therefore request an invalid partial
//! layer. [`plan_replica_layer`] performs the HPA utilization calculation in
//! per-shard units and always returns a valid whole-layer total.
//!
//! This module deliberately plans but does not apply a membership change.
//! `raft-runtime` currently has static membership; a controller must complete a
//! Raft membership transition before patching the StatefulSet replica count.
//! Storage pressure is a separate axis: [`plan_shard_split`] plans one physical
//! shard at a time from the busiest shard's durable bytes. The service still
//! owns its domain-safe routing-map cutover and data movement; this library
//! never treats adding StatefulSet pods as a completed shard split.

pub use crate::domain::capacity::{
    plan_replica_layer, plan_shard_split, resource_request_or_default, ObservedShardUsage,
    ObservedUtilization, ReplicaLayerError, ReplicaLayerPlan, ReplicaLayerPolicy, ShardSplitError,
    ShardSplitPlan, ShardSplitPolicy, DEFAULT_CPU_REQUEST, DEFAULT_MEMORY_REQUEST,
    DEFAULT_SHARD_SPLIT_THRESHOLD_BYTES,
};
