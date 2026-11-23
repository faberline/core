//! The generic reconcile loop. Watches a [`ManagedService`] CR cluster-wide, or
//! in a listed set of namespaces with [`run_in_namespaces`]; for
//! each, server-side-applies the rendered child objects as the field manager
//! `S::MANAGER`, then writes back its status. Only the Lease holder applies
//! (leader-election gate), so `replicas > 1` is safe. Child objects are applied
//! generically as [`DynamicObject`]s keyed by GVK — no compile-time type per
//! kind. Lifted from lumen's `service_k8s::reconcile`, generic over `S`.

pub use crate::app::operator::{reconcile_once, run, run_in_namespaces};
pub use crate::interfaces::operator::{Error, WatchScope};
