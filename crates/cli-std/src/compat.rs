//! Old public module paths, kept as re-exports.

pub mod artifact;
pub mod chainable;
#[cfg(feature = "k8s")]
pub mod connect;
pub mod issue;
pub mod llm;
#[cfg(feature = "registry")]
pub mod registry;
/// Deprecated alias of [`issue`] — kept until keep/loom/lumen adopt `issue`.
pub mod report_issue;
pub mod upgrade;
