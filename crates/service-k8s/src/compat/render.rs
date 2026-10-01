//! The sharded-HA render toolkit: a [`RenderCtx`] carrying the per-service
//! identity (app/manager/GVK/name/ns/owner) plus helpers that emit the common
//! k8s objects — labels/selector/meta, ServiceAccount, headless + client
//! Services, PodDisruptionBudget, CronJobs, and [`sharded_statefulset`]: the
//! downward-API StatefulSet whose env feeds
//! `raft_runtime::cluster::ClusterTopology::from_env`.
//!
//! Lifted + parameterized from lumen's `service_k8s::render` helpers. A service
//! keeps its own service-specific rendering and calls these for the shared
//! shapes.
//!
//! The layout is lopsided on purpose (#1849). Pod composition and the stateless
//! Deployment shape live in [`common`] and [`deployment`], but the StatefulSet
//! helpers and the ordinary children stayed at the root: their callers were
//! already deployed against these paths, and moving them would have made a
//! rendering refactor into a breaking change for every adopter at once. So a
//! helper's depth here records when it arrived, not how shared it is — do not
//! read the root as legacy, and do not "finish" the split without moving the
//! callers in the same change.

pub use crate::infrastructure::manifest::{
    client_service, client_service_with_ports, cron_job, dedicated_node_affinity,
    guaranteed_resources, headless_service, headless_service_with_ports, horizontal_pod_autoscaler,
    owner_ref, pdb, requested_resources, restricted_container_security_context,
    restricted_pod_security_context, service_account, service_statefulset,
    service_statefulset_with_service_links, sharded_statefulset, token_registry_mount,
    token_registry_volume, ClusterRoleBindingPlan, ContainerPlan, CronJob, CronJobPlan,
    DaemonSetPlan, DeploymentPlan, FqdnMatchPlan, FqdnNetworkPolicyPlan, HorizontalPodAutoscaler,
    LabelSet, NetworkPeerPlan, NetworkPolicyPlan, NetworkPortPlan, NetworkRulePlan,
    PersistentVolumeClaimPlan, PodDisruptionBudgetPlan, PodPlan, PodRuntimePolicy, RbacRulePlan,
    RenderCtx, RoleBindingPlan, RolePlan, ServiceAccountPlan, ServiceAccountSubjectPlan,
    ServicePlan, ServicePortPlan, ServiceStatefulSet, ShardedStatefulSet, StatefulSetPlan,
    TokenRegistryProjection, TokenRegistrySource, WorkloadPlan, WorkloadPlanError,
    WorkloadVolumeClaim, DEFAULT_TOKEN_REGISTRY_CSI_DRIVER, ENV_POD_NAME, ENV_POD_NAMESPACE,
    ENV_REPLICAS_PER_SHARD, ENV_SHARD_COUNT, ENV_VOTER_COUNT,
};

pub mod common {
    pub use crate::infrastructure::manifest::common::{
        apply_termination_budget, apply_termination_contract, client_service,
        client_service_with_ports, cron_job, guaranteed_resources, horizontal_pod_autoscaler,
        network_policy, owner_ref, pdb, requested_resources, service_account, CronJob,
        HorizontalPodAutoscaler, NetworkPolicy, RenderCtx, ServicePodTemplate,
    };
}

pub mod deployment {
    pub use crate::infrastructure::manifest::deployment::{service_deployment, ServiceDeployment};
}

pub mod projected_token {
    pub use crate::infrastructure::manifest::projected_token::{
        ProjectedServiceAccountToken, DEFAULT_TOKEN_FILE, MINIMUM_EXPIRATION_SECONDS,
    };
}

pub mod rbac {
    pub use crate::infrastructure::manifest::rbac::{
        cluster_role_binding, first_wildcard, role, role_binding, ClusterRoleBinding, NamedRule,
        Role, RoleBinding, RoleSubject, ServiceAccountSubject,
    };
}

pub mod stateful_instance {
    pub use crate::infrastructure::manifest::stateful_instance::{
        render_compat_service_statefulset, stateful_instance, ExistingClaim, StatefulInstanceError,
        StatefulInstancePlan, StatefulInstanceRender, StatefulStorageAttachment,
        VolumeClaimTemplate,
    };
}

pub mod workload_plan {
    pub use crate::infrastructure::manifest::workload_plan::{
        ClusterRoleBindingPlan, ContainerPlan, CronJobPlan, DaemonSetPlan, DeploymentPlan,
        FqdnMatchPlan, FqdnNetworkPolicyPlan, LabelSet, NetworkPeerPlan, NetworkPolicyPlan,
        NetworkPortPlan, NetworkRulePlan, PersistentVolumeClaimPlan, PodDisruptionBudgetPlan,
        PodPlan, PodRuntimePolicy, RbacRulePlan, RoleBindingPlan, RolePlan, ServiceAccountPlan,
        ServiceAccountSubjectPlan, ServicePlan, ServicePortPlan, StatefulSetPlan, WorkloadPlan,
        WorkloadPlanError,
    };
}
