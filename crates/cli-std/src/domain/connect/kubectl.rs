//! What `connect` needs from the cluster: objects read through kubectl.

/// Reads cluster objects through kubectl.
pub(crate) trait Kubectl {
    /// `kubectl get <resource> <name> -n <namespace> -o json`, through
    /// `--context` when one is given, parsed.
    fn get_json(
        &self,
        context: Option<&str>,
        resource: &str,
        name: &str,
        namespace: &str,
    ) -> Result<serde_json::Value, KubectlError>;

    /// The decoded `data.<key>` value of Secret `secret`.
    fn secret_data(
        &self,
        context: Option<&str>,
        namespace: &str,
        secret: &str,
        key: &str,
    ) -> Result<Vec<u8>, KubectlError>;
}

/// A cluster object that could not be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum KubectlError {
    /// kubectl could not be run.
    #[error("run kubectl get {resource} {name} -n {namespace}")]
    Run {
        resource: String,
        name: String,
        namespace: String,
        #[source]
        source: std::io::Error,
    },
    /// kubectl exited with an error; `stderr` is its error output.
    #[error("kubectl get {resource} {name} -n {namespace} failed: {stderr}")]
    Failed {
        resource: String,
        name: String,
        namespace: String,
        stderr: String,
    },
    /// kubectl's output is not JSON.
    #[error("parse kubectl get {resource} {name} JSON")]
    Parse {
        resource: String,
        name: String,
        #[source]
        source: serde_json::Error,
    },
    /// The Secret has no such data key.
    #[error("secret has no data key `{key}`")]
    MissingKey { key: String },
    /// The data value is not valid base64.
    #[error("base64-decode secret data")]
    Decode(#[source] Box<dyn std::error::Error + Send + Sync>),
}
