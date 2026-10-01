use std::collections::HashMap;
use std::process::{Child, Command};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::domain::connect::TokenClaims;

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
    let mut cmd = Command::new("kubectl");
    if let Some(ctx) = context {
        cmd.args(["--context", ctx]);
    }
    cmd.args(["get", resource, name, "-n", namespace, "-o", "json"]);
    let output = cmd
        .output()
        .with_context(|| format!("run kubectl get {resource} {name} -n {namespace}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "kubectl get {resource} {name} -n {namespace} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("parse kubectl get {resource} {name} JSON"))
}

/// Pure: decode a Kubernetes Secret's `data.<key>` (base64) field into raw
/// bytes. `kubectl get secret -o json` always base64-encodes `.data`.
pub fn secret_data_bytes(secret_json: &serde_json::Value, key: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let encoded = secret_json["data"][key]
        .as_str()
        .with_context(|| format!("secret has no data key `{key}`"))?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("base64-decode secret data")
}

/// The bearer-secret half of a token-registry document, whichever shape it is
/// written in.
///
/// A registry may be namespaced — `{"tokens": {…}, "identities": {…}}` — or the
/// older flat map of secret to claims. Only `tokens` is a presentable
/// credential: an `identities` entry names an email an external provider
/// vouches for, and a CLI cannot present an email as a bearer token.
///
/// The discriminator has to match `service_auth::Registry::parse` exactly, or a
/// registry the server reads one way is read the other way here. It is
/// duplicated rather than shared because `service-auth` depends on `cli-std`,
/// not the reverse; the two are pinned together by
/// `both_registry_shapes_resolve_the_same_token`.
pub(crate) fn bearer_secrets(bytes: &[u8]) -> Result<HashMap<String, TokenClaims>> {
    let doc: serde_json::Value =
        serde_json::from_slice(bytes).context("parse token-registry.json")?;
    let map = doc
        .as_object()
        .context("token-registry.json must be a JSON object")?;
    // Namespaced only when every key is a section name AND no top-level value
    // is itself a claims object — otherwise a flat registry whose single secret
    // is literally spelled `tokens` would be misread as a section.
    let namespaced = map.keys().all(|key| key == "tokens" || key == "identities")
        && !map.values().any(|value| value.get("subject").is_some());
    let tokens = if namespaced {
        map.get("tokens").cloned().unwrap_or(serde_json::json!({}))
    } else {
        doc
    };
    serde_json::from_value(tokens).context("parse token-registry.json bearer secrets")
}

#[cfg(test)]
mod tests;
