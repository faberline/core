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

fn workload(name: &str, kind: WorkloadKind) -> ExtraWorkload {
    ExtraWorkload {
        name: name.to_string(),
        kind,
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

struct TwoWorkloads;
impl ServiceProfile for TwoWorkloads {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![
            workload("collector", WorkloadKind::Deployment),
            workload("archive", WorkloadKind::CronJob),
        ]
    }
}

struct Collides;
impl ServiceProfile for Collides {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![workload("config", WorkloadKind::Deployment)]
    }
}

struct CollidesLater;
impl ServiceProfile for CollidesLater {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![
            workload("collector", WorkloadKind::Deployment),
            workload("alerts", WorkloadKind::DaemonSet),
        ]
    }
}

struct Repeats;
impl ServiceProfile for Repeats {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![
            workload("collector", WorkloadKind::Deployment),
            workload("collector", WorkloadKind::StatefulSet),
        ]
    }
}

struct BadName;
impl ServiceProfile for BadName {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![workload("Bad_Name", WorkloadKind::Deployment)]
    }
}

struct Len30;
impl ServiceProfile for Len30 {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![workload(&"a".repeat(30), WorkloadKind::Deployment)]
    }
}

struct Len31;
impl ServiceProfile for Len31 {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![workload(&"a".repeat(31), WorkloadKind::Deployment)]
    }
}

struct LeadingDash;
impl ServiceProfile for LeadingDash {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![workload("-a", WorkloadKind::Deployment)]
    }
}

struct EmptyName;
impl ServiceProfile for EmptyName {
    const KIND: &'static str = "Sift";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![workload("", WorkloadKind::Deployment)]
    }
}

struct BadKindAndBadName;
impl ServiceProfile for BadKindAndBadName {
    const KIND: &'static str = "";
    const PREFIX: &'static str = "SIFT";
    type Config = ();
    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }
    fn alerts() -> Vec<AlertRule> {
        vec![]
    }
    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![workload("Bad_Name", WorkloadKind::Deployment)]
    }
}

#[test]
fn a_profile_declares_the_main_statefulset_and_extra_workloads() {
    assert_eq!(
        TwoWorkloads::extra_workloads(),
        vec![
            workload("collector", WorkloadKind::Deployment),
            workload("archive", WorkloadKind::CronJob),
        ]
    );
    assert_eq!(validate_profile::<TwoWorkloads>(), Ok(()));
}

#[test]
fn a_profile_with_no_extra_workload_is_unchanged() {
    assert!(Ok1::extra_workloads().is_empty());
    assert_eq!(validate_profile::<Ok1>(), Ok(()));
}

#[test]
fn a_name_that_collides_with_a_core_child_is_refused() {
    assert_eq!(
        validate_profile::<Collides>(),
        Err(ProfileError::WorkloadNameCollision {
            name: "config".to_string()
        })
    );
    assert_eq!(
        validate_profile::<CollidesLater>(),
        Err(ProfileError::WorkloadNameCollision {
            name: "alerts".to_string()
        })
    );
}

#[test]
fn every_reserved_name_is_refused() {
    assert_eq!(RESERVED_WORKLOAD_NAMES.len(), 4);
    for name in ["alerts", "config", "headless", "server"] {
        assert!(RESERVED_WORKLOAD_NAMES.contains(&name));
    }
}

#[test]
fn a_repeated_workload_name_is_refused() {
    assert_eq!(
        validate_profile::<Repeats>(),
        Err(ProfileError::DuplicateWorkload {
            name: "collector".to_string()
        })
    );
}

#[test]
fn an_invalid_workload_name_is_refused() {
    assert_eq!(
        validate_profile::<BadName>(),
        Err(ProfileError::InvalidWorkloadName {
            name: "Bad_Name".to_string()
        })
    );
}

#[test]
fn the_workload_name_rule_has_edges() {
    assert_eq!(validate_profile::<Len30>(), Ok(()));
    assert_eq!(
        validate_profile::<Len31>(),
        Err(ProfileError::InvalidWorkloadName {
            name: "a".repeat(31)
        })
    );
    assert_eq!(
        validate_profile::<LeadingDash>(),
        Err(ProfileError::InvalidWorkloadName {
            name: "-a".to_string()
        })
    );
    assert_eq!(
        validate_profile::<EmptyName>(),
        Err(ProfileError::InvalidWorkloadName {
            name: String::new()
        })
    );
}

#[test]
fn the_core_check_order_comes_before_the_workload_check() {
    assert_eq!(
        validate_profile::<BadKindAndBadName>(),
        Err(ProfileError::EmptyKind)
    );
}
