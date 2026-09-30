//! The composition root: wiring that may use every layer.
//!
//! The public `Reconciler::new` hands the certificate reconciler the rcgen key
//! generator and the X.509 leaf parser.

#[cfg(feature = "certificate")]
mod certificate;
