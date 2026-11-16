//! The service profile contract and its pure validation, for apps (ADR 0002 D4).

pub use crate::domain::profile::{
    validate_profile, AlertRule, Capabilities, Partitioning, ProfileError, ReadScaling,
    ServiceProfile,
};

pub use crate::interfaces::service_spec::{
    crd_for, AuthMode, AuthSpec, BackupSpec, ImageSpec, MonitoringSpec, PeerTlsSpec, PlacementSpec,
    ReplicasSpec, ServiceSpec, StorageSpec,
};
