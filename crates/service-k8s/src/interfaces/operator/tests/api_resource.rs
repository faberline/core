use crate::interfaces::operator::children::{api_resource, plural_for};

/// The naive `lower(kind) + "s"` fallback is right for every kind the
/// toolkit rendered until now, and silently wrong for a kind ending in
/// `-y`. A wrong plural is invisible until an apply 404s against a live
/// apiserver — exactly the class of bug a unit test should catch instead
/// of a cluster run.
#[test]
fn irregular_plurals_are_pinned_rather_than_derived() {
    assert_eq!(plural_for("NetworkPolicy"), "networkpolicies");
    assert_ne!(
        plural_for("NetworkPolicy"),
        "networkpolicys",
        "the fallback's plural for a -y kind is not served by any apiserver"
    );

    // Kinds the fallback happens to get right are still pinned, so a future
    // rewrite of the table cannot quietly drop one.
    for (kind, plural) in [
        ("PodDisruptionBudget", "poddisruptionbudgets"),
        ("HorizontalPodAutoscaler", "horizontalpodautoscalers"),
        ("StatefulSet", "statefulsets"),
    ] {
        assert_eq!(plural_for(kind), plural);
    }

    // Unlisted regular kinds must keep flowing through the fallback;
    // pinning every kind by hand is how the table goes stale.
    assert_eq!(plural_for("Secret"), "secrets");
}

#[test]
fn api_resource_splits_group_and_version_for_a_networking_child() {
    let ar = api_resource("networking.k8s.io/v1", "NetworkPolicy");
    assert_eq!(ar.group, "networking.k8s.io");
    assert_eq!(ar.version, "v1");
    assert_eq!(ar.plural, "networkpolicies");

    // Core-group kinds carry an empty group, not the literal "v1".
    let core = api_resource("v1", "ConfigMap");
    assert_eq!(core.group, "");
    assert_eq!(core.version, "v1");
}
