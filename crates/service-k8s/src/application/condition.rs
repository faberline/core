//! Status conditions at the application edge: the wall-clock read that stamps
//! them and, with the `controller` feature, the condition step of a reconcile
//! pass.

#[cfg(feature = "controller")]
pub(crate) mod status_patch;

/// Now, in the RFC3339 form Kubernetes expects in `lastTransitionTime`.
/// Second precision: metav1 timestamps carry no sub-second component, and
/// emitting one makes the API server rewrite the value on every write.
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests;
