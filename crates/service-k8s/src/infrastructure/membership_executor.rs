//! The Kubernetes adapter of the membership port: StatefulSet scale and PVC
//! delete go to the apiserver; the five Raft calls go to a [`RaftAdminClient`].
//!
//! The adapter makes exactly one Kubernetes or admin call per method. It does
//! not check leadership; the caller does (`run_membership_step_as_leader`).

use futures::future::BoxFuture;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use kube::api::{Api, DeleteParams, Patch, PatchParams};
use kube::Client;
use serde_json::json;

use crate::domain::membership_port::{
    MembershipError, MembershipExecutor, RaftAdminClient, StepResult,
};

/// How a shard and a replica map to Kubernetes names. The per-shard naming is
/// owned by the render, not by this adapter.
pub struct MembershipNaming {
    /// The StatefulSet name of a shard.
    pub statefulset: Box<dyn Fn(u32) -> String + Send + Sync>,
    /// The PVC name of a replica in a shard.
    pub pvc: Box<dyn Fn(u32, u32) -> String + Send + Sync>,
}

/// Runs membership steps against a cluster and a Raft admin client.
pub struct KubeMembershipExecutor<A> {
    client: Client,
    namespace: String,
    admin: A,
    naming: MembershipNaming,
}

impl<A> KubeMembershipExecutor<A> {
    /// Builds an executor for the StatefulSets and PVCs in `namespace`.
    pub fn new(client: Client, namespace: String, admin: A, naming: MembershipNaming) -> Self {
        Self {
            client,
            namespace,
            admin,
            naming,
        }
    }
}

impl<A: RaftAdminClient> MembershipExecutor for KubeMembershipExecutor<A> {
    fn scale_pods<'a>(
        &'a self,
        shard: u32,
        replicas: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            let name = (self.naming.statefulset)(shard);
            Api::<StatefulSet>::namespaced(self.client.clone(), &self.namespace)
                .patch_scale(
                    &name,
                    &PatchParams::default(),
                    &Patch::Merge(json!({"spec": {"replicas": replicas}})),
                )
                .await
                .map_err(|e| {
                    MembershipError::new(format!("scale statefulset {name} to {replicas}: {e}"))
                })?;
            Ok(StepResult::Applied)
        })
    }

    fn add_learner<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move { self.admin.add_learner(shard, replica).await })
    }

    fn promote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move { self.admin.promote(shard, replica).await })
    }

    fn transfer_leadership<'a>(
        &'a self,
        shard: u32,
        from: u32,
        to: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move { self.admin.transfer_leadership(shard, from, to).await })
    }

    fn demote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move { self.admin.demote(shard, replica).await })
    }

    fn remove_member<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move { self.admin.remove_member(shard, replica).await })
    }

    fn delete_pvc<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            let name = (self.naming.pvc)(shard, replica);
            let api =
                Api::<PersistentVolumeClaim>::namespaced(self.client.clone(), &self.namespace);
            match api.delete(&name, &DeleteParams::default()).await {
                Ok(_) => Ok(StepResult::Applied),
                // Already gone: the step must be repeatable.
                Err(kube::Error::Api(e)) if e.code == 404 => Ok(StepResult::Applied),
                Err(e) => Err(MembershipError::new(format!("delete pvc {name}: {e}"))),
            }
        })
    }
}
