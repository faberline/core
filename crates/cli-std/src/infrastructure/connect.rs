use std::process::{Child, Command};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::domain::connect::kubectl::{Kubectl, KubectlError};

/// RAII child-process guard: kills + reaps on drop so a spawned `kubectl
/// port-forward` never survives its wrapped command. Prior art:
/// `projects/preview/tests/kind_lifecycle.rs`'s `ChildGuard`, generalized
/// here over any `std::process::Command` so it is unit-testable with a fake
/// child instead of requiring a real cluster.
pub struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    pub fn spawn(command: &mut Command) -> Result<Self> {
        let child = command.spawn().context("spawn child process")?;
        Ok(Self { child })
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Bind an ephemeral local port and immediately release it, returning the
/// number `kubectl port-forward` should target. There is an inherent
/// TOCTOU race (someone else could bind it first), the same tradeoff
/// `projects/preview/tests/kind_lifecycle.rs::free_local_port` makes.
pub fn free_local_port() -> Result<u16> {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).context("bind ephemeral local port")?;
    Ok(listener.local_addr().context("read local addr")?.port())
}

/// Poll `127.0.0.1:port` until a TCP connect succeeds or `timeout` elapses —
/// the port-forward readiness gate: no fixed sleep, no dependency on
/// kubectl's own stdout.
pub fn wait_for_local_port_ready(port: u16, timeout: Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("port-forward to 127.0.0.1:{port} never became ready within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Run `kubectl get <resource> <name> -n <namespace> -o json` (optionally
/// through `--context`) and parse the result.
pub fn kubectl_get_json(
    context: Option<&str>,
    resource: &str,
    name: &str,
    namespace: &str,
) -> Result<serde_json::Value> {
    Ok(run_kubectl_get(context, resource, name, namespace)?)
}

/// Pure: decode a Kubernetes Secret's `data.<key>` (base64) field into raw
/// bytes. `kubectl get secret -o json` always base64-encodes `.data`.
pub fn secret_data_bytes(secret_json: &serde_json::Value, key: &str) -> Result<Vec<u8>> {
    Ok(decode_secret_data(secret_json, key)?)
}

/// The [`Kubectl`] port over the `kubectl` binary on `PATH`.
pub(crate) struct KubectlCli;

impl Kubectl for KubectlCli {
    fn get_json(
        &self,
        context: Option<&str>,
        resource: &str,
        name: &str,
        namespace: &str,
    ) -> Result<serde_json::Value, KubectlError> {
        run_kubectl_get(context, resource, name, namespace)
    }

    fn secret_data(
        &self,
        context: Option<&str>,
        namespace: &str,
        secret: &str,
        key: &str,
    ) -> Result<Vec<u8>, KubectlError> {
        let secret_json = run_kubectl_get(context, "secret", secret, namespace)?;
        decode_secret_data(&secret_json, key)
    }
}

fn run_kubectl_get(
    context: Option<&str>,
    resource: &str,
    name: &str,
    namespace: &str,
) -> Result<serde_json::Value, KubectlError> {
    let mut cmd = Command::new("kubectl");
    if let Some(ctx) = context {
        cmd.args(["--context", ctx]);
    }
    cmd.args(["get", resource, name, "-n", namespace, "-o", "json"]);
    let output = cmd.output().map_err(|source| KubectlError::Run {
        resource: resource.to_string(),
        name: name.to_string(),
        namespace: namespace.to_string(),
        source,
    })?;
    if !output.status.success() {
        return Err(KubectlError::Failed {
            resource: resource.to_string(),
            name: name.to_string(),
            namespace: namespace.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|source| KubectlError::Parse {
        resource: resource.to_string(),
        name: name.to_string(),
        source,
    })
}

fn decode_secret_data(secret_json: &serde_json::Value, key: &str) -> Result<Vec<u8>, KubectlError> {
    use base64::Engine;
    let encoded = secret_json["data"][key]
        .as_str()
        .ok_or_else(|| KubectlError::MissingKey {
            key: key.to_string(),
        })?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| KubectlError::Decode(Box::new(e)))
}

#[cfg(test)]
mod tests;
