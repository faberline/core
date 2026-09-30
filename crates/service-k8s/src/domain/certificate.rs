//! The certificate lifecycle's domain: profiles, the renewal state machine,
//! status facts, projected-state values, the Secret layout, and the ports its
//! adapters implement.

pub(crate) mod digest;
pub(crate) mod issuer;
pub(crate) mod profile;
pub(crate) mod projection;
pub(crate) mod secret_layout;
pub(crate) mod secret_store;
pub(crate) mod state;
pub(crate) mod status;
