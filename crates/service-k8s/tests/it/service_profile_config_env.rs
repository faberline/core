//! Config flattening and secret mounts reach the public `service_k8s::profile` path (#388).
//! The config code is always built, so there is no feature gate here.

use service_k8s::profile::{
    config_env, AlertRule, Capabilities, Partitioning, ReadScaling, SecretRef, ServiceProfile,
};

struct Profile;

impl ServiceProfile for Profile {
    const KIND: &'static str = "P";
    const PREFIX: &'static str = "P";
    type Config = serde_json::Value;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

#[test]
fn config_flattens_and_mounts_through_the_public_path() {
    let secret = serde_json::to_value(SecretRef {
        secret_name: "s".into(),
        key: "k".into(),
    })
    .expect("secret ref serializes");
    let config = serde_json::json!({"mode": "fast", "key": secret});

    let env = config_env::<Profile>(&config).expect("config flattens");

    assert_eq!(env.data["P_MODE"], "fast");
    assert_eq!(env.data["P_KEY"], "/etc/core/secrets/p-key/k");
    assert_eq!(env.secret_mounts.len(), 1);
}
