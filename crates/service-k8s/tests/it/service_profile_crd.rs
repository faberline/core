use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use service_k8s::profile::{AlertRule, Capabilities, Partitioning, ReadScaling, ServiceProfile};

#[derive(Serialize, Deserialize, JsonSchema)]
struct Config {
    n: u32,
}

struct Probe;

impl ServiceProfile for Probe {
    const KIND: &'static str = "Probe";
    const PREFIX: &'static str = "PROBE";
    type Config = Config;

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
fn the_crd_is_reachable_through_the_public_path() {
    let v = service_k8s::profile::crd_for::<Probe>("p.example").unwrap();
    assert_eq!(v["spec"]["names"]["kind"], "Probe");
}
