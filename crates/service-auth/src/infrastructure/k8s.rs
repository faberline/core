//! Kubernetes adapters: the kube-rs review and TokenRequest transports, the
//! projected token file, and the private-CA verifying client.

#[cfg(feature = "k8s")]
pub(crate) mod kube_backend;
#[cfg(feature = "k8s")]
pub(crate) mod kube_token_minter;
mod projected_token_file;
mod verifying_client;

pub use projected_token_file::{ProjectedTokenError, ProjectedTokenFile};
pub use verifying_client::verifying_client;
