//! Pure values and rules: token-registry claims and token selection, issue
//! bodies, payloads and URLs, and the `cclab.llm.v2` wire types.

#[cfg(feature = "k8s")]
pub(crate) mod connect;
pub(crate) mod issue;
pub(crate) mod llm;
