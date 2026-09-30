//! The generic reconcile loop. Watches a [`ManagedService`] CR cluster-wide; for
//! each, server-side-applies the rendered child objects as the field manager
//! `S::MANAGER`, then writes back its status. Only the Lease holder applies
//! (leader-election gate), so `replicas > 1` is safe. Child objects are applied
//! generically as [`DynamicObject`]s keyed by GVK — no compile-time type per
//! kind. Lifted from lumen's `service_k8s::reconcile`, generic over `S`.

pub use crate::app::operator::{reconcile_once, run};
pub use crate::interfaces::operator::Error;
