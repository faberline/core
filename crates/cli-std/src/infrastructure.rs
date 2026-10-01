//! Adapters: the GitHub and courier HTTP clients, kubectl and local ports,
//! the terminal prompt, and in-place replacement of the running binary.

#[cfg(feature = "online")]
pub(crate) mod confirm;
#[cfg(feature = "k8s")]
pub(crate) mod connect;
#[cfg(feature = "online")]
pub(crate) mod courier;
#[cfg(feature = "online")]
pub(crate) mod github;
#[cfg(feature = "online")]
pub(crate) mod http;
#[cfg(feature = "online")]
pub(crate) mod issue;
#[cfg(feature = "online")]
pub(crate) mod self_install;
