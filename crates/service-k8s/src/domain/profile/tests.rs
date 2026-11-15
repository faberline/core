use super::*;

fn alert(name: &str) -> AlertRule {
    AlertRule {
        name: name.to_string(),
        expression: "x".to_string(),
        for_duration: "x".to_string(),
        severity: "x".to_string(),
        summary: "x".to_string(),
    }
}

/// Declares a unit struct that implements `ServiceProfile` with the given items.
macro_rules! profile {
    ($name:ident, kind = $kind:expr, prefix = $prefix:expr, partitioning = $part:expr, read = $read:expr, alerts = $alerts:expr) => {
        struct $name;
        impl ServiceProfile for $name {
            const KIND: &'static str = $kind;
            const PREFIX: &'static str = $prefix;
            type Config = ();
            fn capabilities() -> Capabilities {
                Capabilities {
                    partitioning: $part,
                    read_scaling: $read,
                }
            }
            fn alerts() -> Vec<AlertRule> {
                $alerts
            }
        }
    };
}

profile!(
    Ok1,
    kind = "Lumen",
    prefix = "LUMEN",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    Learners,
    kind = "Lumen",
    prefix = "LUMEN",
    partitioning = Partitioning::Single,
    read = ReadScaling::Learners,
    alerts = vec![]
);
profile!(
    Bucketed,
    kind = "Lumen",
    prefix = "LUMEN",
    partitioning = Partitioning::Bucketed,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    EmptyKind,
    kind = "",
    prefix = "LUMEN",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    PrefixEmpty,
    kind = "Lumen",
    prefix = "",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    PrefixLower,
    kind = "Lumen",
    prefix = "lumen",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    PrefixDash,
    kind = "Lumen",
    prefix = "LUMEN-X",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    PrefixSpace,
    kind = "Lumen",
    prefix = "LU MEN",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    PrefixDigits,
    kind = "Lumen",
    prefix = "LUMEN_2",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![]
);
profile!(
    DuplicateAlerts,
    kind = "Lumen",
    prefix = "LUMEN",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![alert("A"), alert("B"), alert("A")]
);
profile!(
    DistinctAlerts,
    kind = "Lumen",
    prefix = "LUMEN",
    partitioning = Partitioning::Single,
    read = ReadScaling::None,
    alerts = vec![alert("A"), alert("B")]
);
profile!(
    AllBad,
    kind = "",
    prefix = "lumen",
    partitioning = Partitioning::Bucketed,
    read = ReadScaling::None,
    alerts = vec![]
);

#[test]
fn single_none_profile_validates() {
    assert_eq!(validate_profile::<Ok1>(), Ok(()));
}

#[test]
fn learners_read_scaling_validates() {
    assert_eq!(validate_profile::<Learners>(), Ok(()));
}

#[test]
fn bucketed_is_refused_as_unsupported_capability() {
    assert_eq!(
        validate_profile::<Bucketed>(),
        Err(ProfileError::UnsupportedCapability {
            capability: "partitioning=bucketed"
        })
    );
}

#[test]
fn empty_kind_is_refused() {
    assert_eq!(
        validate_profile::<EmptyKind>(),
        Err(ProfileError::EmptyKind)
    );
}

#[test]
fn bad_prefix_is_refused() {
    assert_eq!(
        validate_profile::<PrefixEmpty>(),
        Err(ProfileError::InvalidPrefix {
            prefix: String::new()
        })
    );
    assert_eq!(
        validate_profile::<PrefixLower>(),
        Err(ProfileError::InvalidPrefix {
            prefix: "lumen".to_string()
        })
    );
    assert_eq!(
        validate_profile::<PrefixDash>(),
        Err(ProfileError::InvalidPrefix {
            prefix: "LUMEN-X".to_string()
        })
    );
    assert_eq!(
        validate_profile::<PrefixSpace>(),
        Err(ProfileError::InvalidPrefix {
            prefix: "LU MEN".to_string()
        })
    );
    assert_eq!(validate_profile::<PrefixDigits>(), Ok(()));
}

#[test]
fn duplicate_alert_name_is_refused() {
    assert_eq!(
        validate_profile::<DuplicateAlerts>(),
        Err(ProfileError::DuplicateAlert {
            name: "A".to_string()
        })
    );
    assert_eq!(validate_profile::<DistinctAlerts>(), Ok(()));
}

#[test]
fn check_order_is_kind_then_prefix_then_alerts_then_capability() {
    assert_eq!(validate_profile::<AllBad>(), Err(ProfileError::EmptyKind));
}
