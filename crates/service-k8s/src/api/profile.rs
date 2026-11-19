//! The service profile contract and its pure validation, for apps (ADR 0002 D4).

pub use crate::domain::profile::{
    validate_profile, AlertRule, Capabilities, ExtraWorkload, Partitioning, ProfileError,
    ReadScaling, ServiceProfile, WorkloadKind, RESERVED_WORKLOAD_NAMES,
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

pub use crate::infrastructure::manifest::instance_render::{
    render_instance, render_operator, render_static, InstanceRenderError, INSTANCE_COMPONENT,
    INSTANCE_DATA_MOUNT_PATH, INSTANCE_DATA_VOLUME, INSTANCE_HTTP_PORT,
};

pub use crate::infrastructure::manifest::derived_rbac::{derive_rbac_rules, DerivedRule};
pub use crate::infrastructure::manifest::operator_install::{
    operator_install, InstallError, InstallOptions,
};

#[cfg(feature = "controller")]
pub use crate::application::operator::profile_service::{ProfileApi, ProfileService};
