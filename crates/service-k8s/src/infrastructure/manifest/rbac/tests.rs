use super::*;

fn labels() -> Value {
    json!({ "app.kubernetes.io/managed-by": "test-operator" })
}

#[test]
fn cluster_role_binding_names_the_role_and_its_service_account_subjects() {
    let subjects = [ServiceAccountSubject {
        namespace: "team-a",
        name: "svc",
    }];
    let obj = cluster_role_binding(ClusterRoleBinding {
        name: "svc-team-a-auth-delegator",
        labels: labels(),
        cluster_role: "system:auth-delegator",
        subjects: &subjects,
    });
    assert_eq!(obj["apiVersion"], "rbac.authorization.k8s.io/v1");
    assert_eq!(obj["kind"], "ClusterRoleBinding");
    assert_eq!(obj["metadata"]["name"], "svc-team-a-auth-delegator");
    assert_eq!(obj["roleRef"]["kind"], "ClusterRole");
    assert_eq!(obj["roleRef"]["name"], "system:auth-delegator");
    assert_eq!(
        obj["subjects"],
        json!([{ "kind": "ServiceAccount", "name": "svc", "namespace": "team-a" }]),
        "the subject carries its own namespace: a cluster-scoped binding has none to inherit"
    );
}

/// The apiserver deletes a cluster-scoped object whose owner reference
/// names a namespaced owner, so emitting one here would be worse than
/// emitting none. The builder has no field for it; this pins the resulting
/// metadata so a later "just add owner refs like the other helpers" cannot
/// pass silently.
#[test]
fn cluster_role_binding_carries_no_owner_reference_and_no_namespace() {
    let subjects = [ServiceAccountSubject {
        namespace: "team-a",
        name: "svc",
    }];
    let obj = cluster_role_binding(ClusterRoleBinding {
        name: "svc-team-a-auth-delegator",
        labels: labels(),
        cluster_role: "system:auth-delegator",
        subjects: &subjects,
    });
    let meta = obj["metadata"].as_object().expect("metadata is an object");
    assert!(
        !meta.contains_key("ownerReferences"),
        "a namespaced owner on a cluster-scoped object is not ignored — it is a delete order"
    );
    assert!(
        !meta.contains_key("namespace"),
        "a cluster-scoped object with a namespace is rejected by the apiserver"
    );
    assert_eq!(meta["labels"], labels());
}

/// Binding to a built-in role rather than to a rendered replacement is the
/// whole point: `system:auth-delegator` is maintained by Kubernetes, and a
/// copy of it would be a second grant to keep in sync with an upstream one
/// nobody watches.
#[test]
fn cluster_role_binding_does_not_render_a_cluster_role() {
    let subjects = [ServiceAccountSubject {
        namespace: "team-a",
        name: "svc",
    }];
    let obj = cluster_role_binding(ClusterRoleBinding {
        name: "b",
        labels: labels(),
        cluster_role: "system:auth-delegator",
        subjects: &subjects,
    });
    assert!(
        obj.get("rules").is_none(),
        "this builder binds an existing role; it never defines one"
    );
}

// ---- namespaced Role / RoleBinding ----------------------------------

/// The narrow grant is the one the type makes you write: naming the
/// objects is a field you cannot skip, and it renders as `resourceNames`.
#[test]
fn a_rule_that_names_its_objects_renders_them() {
    let rules = [NamedRule {
        api_groups: &[""],
        resources: &["serviceaccounts/token"],
        resource_names: &["client"],
        verbs: &["create"],
    }];
    let obj = role(Role {
        name: "client-token-issuer",
        namespace: "team-a",
        labels: labels(),
        rules: &rules,
    });
    assert_eq!(obj["apiVersion"], "rbac.authorization.k8s.io/v1");
    assert_eq!(obj["kind"], "Role");
    assert_eq!(obj["metadata"]["namespace"], "team-a");
    assert_eq!(
        obj["rules"],
        json!([{
            "apiGroups": [""],
            "resources": ["serviceaccounts/token"],
            "verbs": ["create"],
            "resourceNames": ["client"],
        }])
    );
}

/// RBAC spells "every object of this resource" by leaving `resourceNames`
/// out, so an empty slice has to render as an absent key — not as an empty
/// list, which the apiserver reads as a rule matching nothing.
#[test]
fn an_empty_name_list_omits_the_key_rather_than_emitting_an_empty_one() {
    let rules = [NamedRule {
        api_groups: &["example.dev"],
        resources: &["singletons"],
        resource_names: &[],
        verbs: &["get"],
    }];
    let obj = role(Role {
        name: "r",
        namespace: "team-a",
        labels: labels(),
        rules: &rules,
    });
    let rule = obj["rules"][0].as_object().expect("a rule is an object");
    assert!(
        !rule.contains_key("resourceNames"),
        "an empty resourceNames list matches no object at all: `{rule:?}`"
    );
}

/// The two subject kinds render differently on purpose: a `User` needs the
/// RBAC API group and no namespace, a ServiceAccount needs a namespace and
/// no API group. Swapping either is accepted by the apiserver and then
/// silently matches nobody.
#[test]
fn a_user_subject_and_a_service_account_subject_render_their_own_shapes() {
    let subjects = [
        RoleSubject::User("someone@example.com"),
        RoleSubject::ServiceAccount(ServiceAccountSubject {
            namespace: "team-a",
            name: "client",
        }),
    ];
    let obj = role_binding(RoleBinding {
        name: "b",
        namespace: "team-a",
        labels: labels(),
        role: "client-token-issuer",
        subjects: &subjects,
    });
    assert_eq!(obj["roleRef"]["kind"], "Role");
    assert_eq!(obj["roleRef"]["name"], "client-token-issuer");
    assert_eq!(
        obj["subjects"],
        json!([
            {
                "apiGroup": "rbac.authorization.k8s.io",
                "kind": "User",
                "name": "someone@example.com",
            },
            { "kind": "ServiceAccount", "name": "client", "namespace": "team-a" },
        ])
    );
}

/// A username is opaque here. This one is an email, but so is a Google
/// service account, and so are strings this renderer has never seen; the
/// point is that none of them are parsed.
#[test]
fn a_user_name_is_passed_through_untouched() {
    let subjects = [RoleSubject::User(
        "lumen-client@example.iam.gserviceaccount.com",
    )];
    let obj = role_binding(RoleBinding {
        name: "b",
        namespace: "team-a",
        labels: labels(),
        role: "r",
        subjects: &subjects,
    });
    assert_eq!(
        obj["subjects"][0]["name"],
        "lumen-client@example.iam.gserviceaccount.com"
    );
}

#[test]
fn a_wildcard_is_reported_with_the_field_that_carries_it() {
    let rules = [
        NamedRule {
            api_groups: &[""],
            resources: &["serviceaccounts/token"],
            resource_names: &["client"],
            verbs: &["create"],
        },
        NamedRule {
            api_groups: &["example.dev"],
            resources: &["things"],
            resource_names: &["one"],
            verbs: &["get", "*"],
        },
    ];
    let obj = role(Role {
        name: "r",
        namespace: "team-a",
        labels: labels(),
        rules: &rules,
    });
    assert_eq!(first_wildcard(&obj).as_deref(), Some("rules/1/verbs/1"));
}

/// `pods/*` is a wildcard that no equality check against `"*"` would see,
/// and it is the spelling a reviewer is least likely to notice.
#[test]
fn a_wildcard_inside_a_longer_string_is_still_a_wildcard() {
    let rules = [NamedRule {
        api_groups: &[""],
        resources: &["pods/*"],
        resource_names: &["one"],
        verbs: &["get"],
    }];
    let obj = role(Role {
        name: "r",
        namespace: "team-a",
        labels: labels(),
        rules: &rules,
    });
    assert_eq!(first_wildcard(&obj).as_deref(), Some("rules/0/resources/0"));
}

#[test]
fn a_grant_with_no_wildcard_reports_none() {
    let rules = [NamedRule {
        api_groups: &[""],
        resources: &["serviceaccounts/token"],
        resource_names: &["client"],
        verbs: &["create"],
    }];
    let obj = role(Role {
        name: "r",
        namespace: "team-a",
        labels: labels(),
        rules: &rules,
    });
    assert_eq!(first_wildcard(&obj), None);
    let subjects = [RoleSubject::User("someone@example.com")];
    assert_eq!(
        first_wildcard(&role_binding(RoleBinding {
            name: "b",
            namespace: "team-a",
            labels: labels(),
            role: "r",
            subjects: &subjects,
        })),
        None
    );
}
