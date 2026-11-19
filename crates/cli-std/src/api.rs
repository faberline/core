//! Public modules that keep their paths: they hold names the crate root does
//! not re-export.

pub mod artifact;
pub mod chainable;
#[cfg(feature = "k8s")]
pub mod connect;
/// The `dockerfile` verb: render a service Dockerfile from parameters.
pub mod dockerfile;
pub mod issue;
pub mod llm;
#[cfg(feature = "registry")]
pub mod registry;
/// Deprecated alias of [`issue`] — kept until keep/loom/lumen adopt `issue`.
pub mod report_issue;
/// The `spec` verb: print the service API document and run `spec gen`.
pub mod spec;
pub mod upgrade;
