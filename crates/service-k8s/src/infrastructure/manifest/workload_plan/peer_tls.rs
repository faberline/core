//! The peer TLS Secret mount that a service gives its Raft peer listener.

use serde_json::{json, Value};

use super::PodPlan;

#[derive(Clone, Debug)]
pub struct PeerTlsMount {
    app: String,
    env_prefix: String,
    secret_name: String,
}

impl PeerTlsMount {
    /// `app` names the volume (`<app>-peer-tls`) and the mount path
    /// (`/var/run/secrets/<app>-peer`). `env_prefix` is the prefix that
    /// `peer_tls::PeerTlsConfig::from_env` reads, for example `RELAY_PEER`.
    /// `secret_name` is a Secret with `tls.crt`, `tls.key` and `ca.crt`.
    pub fn new(
        app: impl Into<String>,
        env_prefix: impl Into<String>,
        secret_name: impl Into<String>,
    ) -> Self {
        Self {
            app: app.into(),
            env_prefix: env_prefix.into(),
            secret_name: secret_name.into(),
        }
    }

    /// The volume name, `<app>-peer-tls`.
    pub fn volume_name(&self) -> String {
        format!("{}-peer-tls", self.app)
    }

    /// The read-only mount path, `/var/run/secrets/<app>-peer`.
    pub fn mount_path(&self) -> String {
        format!("/var/run/secrets/{}-peer", self.app)
    }

    /// The Secret volume that projects the three keys of the peer TLS Secret.
    pub fn volume(&self) -> Value {
        json!({
            "name": self.volume_name(),
            "secret": {
                "secretName": self.secret_name,
                "items": [
                    {"key": "tls.crt", "path": "tls.crt"},
                    {"key": "tls.key", "path": "tls.key"},
                    {"key": "ca.crt", "path": "ca.crt"}
                ]
            }
        })
    }

    /// The read-only container mount of the volume.
    pub fn volume_mount(&self) -> Value {
        json!({
            "name": self.volume_name(),
            "mountPath": self.mount_path(),
            "readOnly": true
        })
    }

    /// The four env vars that `peer_tls::PeerTlsConfig::from_env` reads.
    pub fn env(&self) -> Vec<Value> {
        let path = self.mount_path();
        vec![
            json!({"name": format!("{}_MTLS", self.env_prefix), "value": "on"}),
            json!({
                "name": format!("{}_TLS_CERT", self.env_prefix),
                "value": format!("{path}/tls.crt")
            }),
            json!({
                "name": format!("{}_TLS_KEY", self.env_prefix),
                "value": format!("{path}/tls.key")
            }),
            json!({
                "name": format!("{}_TLS_CA", self.env_prefix),
                "value": format!("{path}/ca.crt")
            }),
        ]
    }

    /// Adds the volume to `pod.runtime.volumes`, the mount to
    /// `pod.container.volume_mounts` and the env to `pod.container.env`.
    pub fn apply(&self, pod: &mut PodPlan) {
        pod.runtime.volumes.push(self.volume());
        pod.container.volume_mounts.push(self.volume_mount());
        pod.container.env.extend(self.env());
    }
}
