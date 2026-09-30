//! The composition root: wiring that may use every layer.
//!
//! The public `Reconciler::new` hands the certificate reconciler the rcgen key
//! generator and the X.509 leaf parser. The operator entry points `run` and
//! `reconcile_once` build the kube client and the Lease adapter and start the
//! controller loop.

#[cfg(feature = "certificate")]
mod certificate;
#[cfg(feature = "controller")]
pub(crate) mod operator;
