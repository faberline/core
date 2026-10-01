//! Adapters: registry file loading, the tracing audit sink, Google's HTTP
//! endpoints, and the Kubernetes review/TokenRequest
//! transports, projected token file and verifying client.

pub(crate) mod google;
pub(crate) mod k8s;
mod registry;
mod tracing_sink;

pub use registry::{load_registry, load_registry_file, load_registry_files, RegistrySource};
pub use tracing_sink::TracingAuthEventSink;
