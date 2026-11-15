//! A service profile reaches the public `service_k8s::profile` path (#386).
//! The profile code is always built, so there is no feature gate here.

use service_k8s::profile::{
    validate_profile, AlertRule, Capabilities, Partitioning, ProfileError, ReadScaling,
    ServiceProfile,
};

struct SingleShard;

impl ServiceProfile for SingleShard {
    const KIND: &'static str = "Lumen";
    const PREFIX: &'static str = "LUMEN";
    type Config = ();

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

struct BucketedShards;

impl ServiceProfile for BucketedShards {
    const KIND: &'static str = "Lumen";
    const PREFIX: &'static str = "LUMEN";
    type Config = ();

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Bucketed,
            read_scaling: ReadScaling::None,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

#[test]
fn a_single_shard_profile_validates_through_the_public_path() {
    assert_eq!(validate_profile::<SingleShard>(), Ok(()));
}

#[test]
fn a_bucketed_profile_is_refused_through_the_public_path() {
    assert_eq!(
        validate_profile::<BucketedShards>(),
        Err(ProfileError::UnsupportedCapability {
            capability: "partitioning=bucketed"
        })
    );
}
