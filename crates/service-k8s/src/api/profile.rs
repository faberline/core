//! The service profile contract and its pure validation, for apps (ADR 0002 D4).

pub use crate::domain::profile::{
    validate_profile, AlertRule, Capabilities, Partitioning, ProfileError, ReadScaling,
    ServiceProfile,
};

pub use crate::interfaces::service_spec::{
    crd_for, AuthMode, AuthSpec, BackupSpec, ImageSpec, MonitoringSpec, PeerTlsSpec, PlacementSpec,
    ReplicasSpec, ServiceSpec, StorageSpec,
};

pub use crate::domain::config_env::{
    config_env, config_map, env_from, secret_volume_mounts, secret_volumes, ConfigEnv,
    ConfigEnvError, SecretMount, SecretRef, SECRET_MOUNT_ROOT,
};

pub use crate::domain::instance_conditions::{
    instance_conditions, DomainReadiness, InstanceFacts, STANDARD_CONDITION_TYPES,
};
