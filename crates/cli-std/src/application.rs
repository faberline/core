//! Use cases and the API the sub-contexts publish: the tool identity, llm
//! topics, the issue verbs, self-upgrade, connect token resolution, the
//! chainable-output check and the subcommand registry.

pub(crate) mod chainable;
#[cfg(feature = "k8s")]
pub(crate) mod connect;
pub(crate) mod issue;
pub(crate) mod llm;
#[cfg(feature = "registry")]
pub(crate) mod registry;
pub(crate) mod tool_info;
pub(crate) mod upgrade;

#[cfg(all(test, feature = "online"))]
mod port_tests;
