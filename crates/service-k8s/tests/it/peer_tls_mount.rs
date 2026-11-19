use serde_json::json;
use service_k8s::render::{ContainerPlan, PeerTlsMount, PodPlan, PodRuntimePolicy};

fn mount() -> PeerTlsMount {
    PeerTlsMount::new("relay", "RELAY_PEER", "relay-peer-secret")
}

fn pod() -> PodPlan {
    PodPlan::new(
        "serving",
        ContainerPlan::new("relay", "relay:1", vec![]),
        PodRuntimePolicy::restricted("sa", json!({})),
    )
}

#[test]
fn volume_projects_the_three_keys_of_the_secret() {
    assert_eq!(
        mount().volume(),
        json!({
            "name": "relay-peer-tls",
            "secret": {
                "secretName": "relay-peer-secret",
                "items": [
                    {"key": "tls.crt", "path": "tls.crt"},
                    {"key": "tls.key", "path": "tls.key"},
                    {"key": "ca.crt", "path": "ca.crt"}
                ]
            }
        })
    );
}

#[test]
fn mount_is_read_only_under_the_app_path() {
    assert_eq!(
        mount().volume_mount(),
        json!({
            "name": "relay-peer-tls",
            "mountPath": "/var/run/secrets/relay-peer",
            "readOnly": true
        })
    );
    assert_eq!(mount().mount_path(), "/var/run/secrets/relay-peer");
}

#[test]
fn env_matches_the_peer_tls_loader_names() {
    assert_eq!(
        mount().env(),
        vec![
            json!({"name": "RELAY_PEER_MTLS", "value": "on"}),
            json!({"name": "RELAY_PEER_TLS_CERT", "value": "/var/run/secrets/relay-peer/tls.crt"}),
            json!({"name": "RELAY_PEER_TLS_KEY", "value": "/var/run/secrets/relay-peer/tls.key"}),
            json!({"name": "RELAY_PEER_TLS_CA", "value": "/var/run/secrets/relay-peer/ca.crt"}),
        ]
    );
}

#[test]
fn another_app_gets_its_own_names() {
    let defer = PeerTlsMount::new("defer", "DEFER_PEER", "s");
    assert_eq!(defer.volume_name(), "defer-peer-tls");
    assert_eq!(defer.mount_path(), "/var/run/secrets/defer-peer");
    assert_eq!(defer.env()[1]["name"], "DEFER_PEER_TLS_CERT");
}

#[test]
fn apply_adds_volume_mount_and_env_to_the_pod() {
    let mut pod = pod();
    mount().apply(&mut pod);
    assert_eq!(pod.runtime.volumes, vec![mount().volume()]);
    assert_eq!(pod.container.volume_mounts, vec![mount().volume_mount()]);
    assert_eq!(pod.container.env, mount().env());
}

#[test]
fn apply_keeps_existing_entries() {
    let mut pod = pod();
    pod.container.env.push(json!({"name": "A", "value": "1"}));
    pod.runtime.volumes.push(json!({"name": "v"}));
    mount().apply(&mut pod);
    assert_eq!(pod.container.env.len(), 5);
    assert_eq!(pod.container.env[0]["name"], "A");
    assert_eq!(pod.runtime.volumes.len(), 2);
    assert_eq!(pod.runtime.volumes[0]["name"], "v");
}
