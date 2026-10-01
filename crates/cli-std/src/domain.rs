//! Pure values and rules: token-registry claims and token selection, issue
//! bodies, payloads and URLs, and the `cclab.llm.v2` wire types. The ports
//! through which the use cases reach kubectl, GitHub, courier, the terminal
//! and the running binary live here too; infrastructure implements them.

#[cfg(feature = "k8s")]
pub(crate) mod connect;
pub(crate) mod issue;
pub(crate) mod llm;
#[cfg(feature = "online")]
pub(crate) mod prompt;
#[cfg(feature = "online")]
pub(crate) mod release;
#[cfg(feature = "online")]
pub(crate) mod remote;
